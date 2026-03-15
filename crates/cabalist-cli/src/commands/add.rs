//! `cabalist-cli add <package>` — Add a dependency.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Result};
use cabalist_parser::edit::{self, EditBatch};

use crate::util;

pub fn run(
    file: &Option<PathBuf>,
    package: &str,
    version: Option<&str>,
    component: &str,
) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;

    let cst = &result.cst;

    // Find the target section.
    let (keyword, name) = util::parse_component_spec(component);
    let section_id = edit::find_section(cst, keyword, name).ok_or_else(|| {
        anyhow::anyhow!(
            "Component '{}' not found in {}",
            component,
            cabal_path.display()
        )
    })?;

    // Find the build-depends field within the section.
    let field_id = edit::find_field(cst, section_id, "build-depends").ok_or_else(|| {
        anyhow::anyhow!(
            "No 'build-depends' field found in component '{}'",
            component
        )
    })?;

    // Format the dependency string.
    let dep_str = match version {
        Some(v) => format!("{package} {v}"),
        None => package.to_string(),
    };

    // Check if the package is already present.
    let ast = cabalist_parser::ast::derive_ast(cst);
    let already_present = ast
        .all_dependencies()
        .iter()
        .any(|d| d.package.eq_ignore_ascii_case(package));
    if already_present {
        bail!(
            "Package '{}' is already in build-depends; use 'cabalist-cli remove' first to replace it",
            package
        );
    }

    // Generate edits (sorted alphabetically by default).
    let edits = edit::add_list_item(cst, field_id, &dep_str, true);
    if edits.is_empty() {
        println!("No changes needed.");
        return Ok(ExitCode::SUCCESS);
    }

    // Apply edits.
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let new_source = batch.apply(&cst.source);

    // Write back.
    std::fs::write(&cabal_path, &new_source)?;

    println!(
        "Added '{}' to {} in {}",
        dep_str,
        component,
        cabal_path.display()
    );
    Ok(ExitCode::SUCCESS)
}
