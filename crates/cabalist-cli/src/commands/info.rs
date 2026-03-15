//! `cabalist-cli info` — Show project summary.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_opinions::config::find_and_load_config;
use cabalist_parser::ast::{derive_ast, Component};
use colored::Colorize;

use crate::util;
use crate::OutputFormat;

pub fn run(file: &Option<PathBuf>, format: OutputFormat) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let ast = derive_ast(&result.cst);

    // Load config for lint settings.
    let project_root = cabal_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = find_and_load_config(project_root);
    let lint_config = config.lints.to_lint_config();

    // Run validation + lints for health summary.
    let validation_diags = cabalist_parser::validate(&result.cst);
    let lints = cabalist_opinions::run_lints(&ast, &lint_config);

    match format {
        OutputFormat::Json => print_json_info(&ast, &validation_diags, &lints),
        OutputFormat::Text => print_text_info(&ast, &validation_diags, &lints),
    }

    Ok(ExitCode::SUCCESS)
}

fn print_text_info(
    ast: &cabalist_parser::ast::CabalFile<'_>,
    validation_diags: &[cabalist_parser::Diagnostic],
    lints: &[cabalist_opinions::Lint],
) {
    println!("{}", "Package Information".bold().underline());
    println!();

    // Basic metadata.
    print_field("name", ast.name.unwrap_or("(unknown)"));
    print_field(
        "version",
        &ast.version
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(unknown)".to_string()),
    );
    print_field("license", ast.license.unwrap_or("(not set)"));
    print_field("synopsis", ast.synopsis.unwrap_or("(not set)"));
    print_field("author", ast.author.unwrap_or("(not set)"));
    print_field("maintainer", ast.maintainer.unwrap_or("(not set)"));
    if let Some(ref cv) = ast.cabal_version {
        print_field("cabal-version", cv.raw);
    }
    if let Some(bt) = ast.build_type {
        print_field("build-type", bt);
    }
    if let Some(hp) = ast.homepage {
        print_field("homepage", hp);
    }
    if let Some(br) = ast.bug_reports {
        print_field("bug-reports", br);
    }

    println!();
    println!("{}", "Components".bold().underline());
    println!();

    let components = ast.all_components();
    if components.is_empty() {
        println!("  (none)");
    } else {
        for comp in &components {
            let fields = comp.fields();
            let dep_count = fields.build_depends.len();
            let module_count = match comp {
                Component::Library(lib) => lib.exposed_modules.len() + fields.other_modules.len(),
                _ => fields.other_modules.len(),
            };
            let name = component_label(comp);
            println!(
                "  {:<35} {} module(s), {} dep(s)",
                name, module_count, dep_count
            );
        }
    }

    // Common stanzas.
    if !ast.common_stanzas.is_empty() {
        println!();
        println!("{}", "Common Stanzas".bold().underline());
        println!();
        for cs in &ast.common_stanzas {
            println!("  {}", cs.name);
        }
    }

    // Flags.
    if !ast.flags.is_empty() {
        println!();
        println!("{}", "Flags".bold().underline());
        println!();
        for flag in &ast.flags {
            let default = match flag.default {
                Some(true) => "True",
                Some(false) => "False",
                None => "(unset)",
            };
            println!("  {:<25} default: {}", flag.name, default);
        }
    }

    // Health summary.
    let error_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Error)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Error)
            .count();
    let warning_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Warning)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Warning)
            .count();
    let info_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Info)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Info)
            .count();

    println!();
    println!("{}", "Health".bold().underline());
    println!();

    if error_count == 0 && warning_count == 0 && info_count == 0 {
        println!("  {}", "Clean — no issues found".green());
    } else {
        if error_count > 0 {
            println!("  {} error(s)", error_count.to_string().red().bold());
        }
        if warning_count > 0 {
            println!("  {} warning(s)", warning_count.to_string().yellow().bold());
        }
        if info_count > 0 {
            println!("  {} suggestion(s)", info_count.to_string().cyan());
        }
        println!("  Run {} for details.", "cabalist-cli check".dimmed());
    }
}

fn print_field(label: &str, value: &str) {
    println!("  {:<18} {}", format!("{label}:").dimmed(), value);
}

fn component_label(comp: &Component<'_, '_>) -> String {
    match comp {
        Component::Library(lib) => match lib.fields.name {
            Some(name) => format!("library {name}"),
            None => "library".to_string(),
        },
        Component::Executable(exe) => {
            format!("executable {}", exe.fields.name.unwrap_or("(unnamed)"))
        }
        Component::TestSuite(ts) => {
            format!("test-suite {}", ts.fields.name.unwrap_or("(unnamed)"))
        }
        Component::Benchmark(bm) => {
            format!("benchmark {}", bm.fields.name.unwrap_or("(unnamed)"))
        }
    }
}

fn print_json_info(
    ast: &cabalist_parser::ast::CabalFile<'_>,
    validation_diags: &[cabalist_parser::Diagnostic],
    lints: &[cabalist_opinions::Lint],
) {
    #[derive(serde::Serialize)]
    struct JsonInfo {
        name: Option<String>,
        version: Option<String>,
        license: Option<String>,
        synopsis: Option<String>,
        author: Option<String>,
        maintainer: Option<String>,
        cabal_version: Option<String>,
        components: Vec<JsonComponent>,
        health: JsonHealth,
    }

    #[derive(serde::Serialize)]
    struct JsonComponent {
        kind: String,
        name: Option<String>,
        modules: usize,
        dependencies: usize,
    }

    #[derive(serde::Serialize)]
    struct JsonHealth {
        errors: usize,
        warnings: usize,
        info: usize,
    }

    let components: Vec<JsonComponent> = ast
        .all_components()
        .iter()
        .map(|comp| {
            let fields = comp.fields();
            let modules = match comp {
                Component::Library(lib) => lib.exposed_modules.len() + fields.other_modules.len(),
                _ => fields.other_modules.len(),
            };
            JsonComponent {
                kind: match comp {
                    Component::Library(_) => "library".to_string(),
                    Component::Executable(_) => "executable".to_string(),
                    Component::TestSuite(_) => "test-suite".to_string(),
                    Component::Benchmark(_) => "benchmark".to_string(),
                },
                name: fields.name.map(|s| s.to_string()),
                modules,
                dependencies: fields.build_depends.len(),
            }
        })
        .collect();

    let error_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Error)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Error)
            .count();
    let warning_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Warning)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Warning)
            .count();
    let info_count = validation_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Info)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Info)
            .count();

    let info = JsonInfo {
        name: ast.name.map(|s| s.to_string()),
        version: ast.version.as_ref().map(|v| v.to_string()),
        license: ast.license.map(|s| s.to_string()),
        synopsis: ast.synopsis.map(|s| s.to_string()),
        author: ast.author.map(|s| s.to_string()),
        maintainer: ast.maintainer.map(|s| s.to_string()),
        cabal_version: ast.cabal_version.as_ref().map(|cv| cv.raw.to_string()),
        components,
        health: JsonHealth {
            errors: error_count,
            warnings: warning_count,
            info: info_count,
        },
    };

    if let Ok(json) = serde_json::to_string_pretty(&info) {
        println!("{json}");
    }
}
