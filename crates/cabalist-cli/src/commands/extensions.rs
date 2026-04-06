//! `cabalist-cli extensions` — List and toggle GHC extensions.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_parser::edit::{self, EditBatch};
use colored::Colorize;

use crate::util;

pub fn run(file: &Option<PathBuf>, toggle: Option<&str>, component: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;

    if let Some(ext_name) = toggle {
        return toggle_extension(&cabal_path, ext_name, component);
    }

    // List mode: show enabled extensions.
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let ast = cabalist_parser::ast::derive_ast(&result.cst);

    let (keyword, name) = util::parse_component_spec(component);
    let components = ast.all_components();
    let comp = components
        .iter()
        .find(|c| match c {
            cabalist_parser::ast::Component::Library(_) => keyword == "library",
            cabalist_parser::ast::Component::Executable(e) => {
                keyword == "executable" && e.fields.name == name
            }
            cabalist_parser::ast::Component::TestSuite(t) => {
                keyword == "test-suite" && t.fields.name == name
            }
            cabalist_parser::ast::Component::Benchmark(b) => {
                keyword == "benchmark" && b.fields.name == name
            }
        });

    let Some(comp) = comp else {
        println!("Component '{}' not found.", component);
        return Ok(ExitCode::SUCCESS);
    };

    let enabled = &comp.fields().default_extensions;
    if enabled.is_empty() {
        println!("No extensions enabled in {}.", component);
    } else {
        println!("{}", component.bold());
        for ext in enabled {
            let info = cabalist_ghc::extensions::extension_info(ext);
            let desc = info.map(|e| e.description.as_str()).unwrap_or("");
            let truncated = if desc.len() > 60 {
                format!("{}...", &desc[..57])
            } else {
                desc.to_string()
            };
            println!("  {:<30} {}", ext.green(), truncated.dimmed());
        }
    }

    // Show available extensions count.
    let all = cabalist_ghc::extensions::load_extensions();
    let enabled_set: std::collections::HashSet<&str> = enabled.iter().copied().collect();
    let available = all.iter().filter(|e| !enabled_set.contains(e.name.as_str())).count();
    println!("\n{} available extensions not enabled.", available);

    Ok(ExitCode::SUCCESS)
}

fn toggle_extension(
    cabal_path: &std::path::Path,
    ext_name: &str,
    component: &str,
) -> Result<ExitCode> {
    let (_source, result) = util::load_and_parse(cabal_path)?;
    let cst = &result.cst;
    let ast = cabalist_parser::ast::derive_ast(cst);

    // Warn if the extension name isn't recognized.
    if cabalist_ghc::extensions::extension_info(ext_name).is_none() {
        let all = cabalist_ghc::extensions::load_extensions();
        // Try case-insensitive match for a helpful suggestion.
        let suggestion = all
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(ext_name))
            .map(|e| e.name.as_str());

        if let Some(correct) = suggestion {
            anyhow::bail!(
                "Unknown extension '{}'. Did you mean '{}'?",
                ext_name,
                correct
            );
        } else {
            eprintln!(
                "warning: '{}' is not a recognized GHC extension",
                ext_name
            );
        }
    }

    let (keyword, name) = util::parse_component_spec(component);
    let section_id = edit::find_section(cst, keyword, name).ok_or_else(|| {
        anyhow::anyhow!("Component '{}' not found", component)
    })?;

    // Check if the extension is currently enabled.
    let is_enabled = ast
        .all_components()
        .iter()
        .find(|c| match c {
            cabalist_parser::ast::Component::Library(_) => keyword == "library",
            cabalist_parser::ast::Component::Executable(e) => {
                keyword == "executable" && e.fields.name == name
            }
            cabalist_parser::ast::Component::TestSuite(t) => {
                keyword == "test-suite" && t.fields.name == name
            }
            cabalist_parser::ast::Component::Benchmark(b) => {
                keyword == "benchmark" && b.fields.name == name
            }
        })
        .map(|c| {
            c.fields()
                .default_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext_name))
        })
        .unwrap_or(false);

    let field_id = edit::find_field(cst, section_id, "default-extensions");

    let edits = if is_enabled {
        // Remove.
        let fid = field_id.ok_or_else(|| anyhow::anyhow!("No default-extensions field"))?;
        edit::remove_list_item(cst, fid, ext_name)
    } else {
        // Add.
        match field_id {
            Some(fid) => edit::add_list_item(cst, fid, ext_name, true),
            None => {
                vec![edit::add_field_to_section(
                    cst,
                    section_id,
                    "default-extensions",
                    ext_name,
                )]
            }
        }
    };

    if edits.is_empty() {
        println!("No changes needed.");
        return Ok(ExitCode::SUCCESS);
    }

    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let new_source = batch.apply(&cst.source);
    std::fs::write(cabal_path, &new_source)?;

    let action = if is_enabled { "Disabled" } else { "Enabled" };
    println!("{action} '{}' in {}", ext_name, component);
    Ok(ExitCode::SUCCESS)
}
