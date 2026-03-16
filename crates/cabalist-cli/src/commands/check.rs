//! `cabalist-cli check` — Run opinionated lints and spec validation.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_opinions::config::find_and_load_config;
use cabalist_parser::ast::derive_ast;
use cabalist_parser::Severity;

use crate::util;
use crate::OutputFormat;

pub fn run(file: &Option<PathBuf>, strict: bool, format: OutputFormat) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (source, result) = util::load_and_parse(&cabal_path)?;

    // Load config for lint settings.
    let project_root = cabal_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = find_and_load_config(project_root);
    let lint_config = config.lints.to_lint_config();

    // Run spec validation.
    let validation_diags = cabalist_parser::validate(&result.cst);

    // Derive AST and run opinionated lints (including filesystem-aware lints).
    let ast = derive_ast(&result.cst);
    let lints = cabalist_opinions::run_all_lints(&ast, &lint_config, project_root);

    match format {
        OutputFormat::Json => print_json_output(&cabal_path, &source, &validation_diags, &lints),
        OutputFormat::Text => {
            // Print validation diagnostics.
            for diag in &validation_diags {
                util::print_diagnostic(&cabal_path, diag, &source);
            }
            // Print opinionated lints.
            for lint in &lints {
                util::print_lint(&cabal_path, lint, &source);
            }
        }
    }

    // Count by severity.
    let error_count = validation_diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == Severity::Error)
            .count();

    let warning_count = validation_diags
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == Severity::Warning)
            .count();

    let info_count = validation_diags
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == Severity::Info)
            .count();

    let total = error_count + warning_count + info_count;

    if matches!(format, OutputFormat::Text) && total > 0 {
        eprintln!();
        eprintln!(
            "Found {} error(s), {} warning(s), {} info(s)",
            error_count, warning_count, info_count
        );
    }

    // Exit codes: 0 = clean, 1 = warnings, 2 = errors.
    if error_count > 0 {
        return Ok(ExitCode::from(2));
    }
    if strict && warning_count > 0 {
        return Ok(ExitCode::from(2));
    }
    if warning_count > 0 {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn print_json_output(
    file: &std::path::Path,
    source: &str,
    validation_diags: &[cabalist_parser::Diagnostic],
    lints: &[cabalist_opinions::Lint],
) {
    #[derive(serde::Serialize)]
    struct JsonOutput {
        file: String,
        diagnostics: Vec<JsonDiagnostic>,
    }

    #[derive(serde::Serialize)]
    struct JsonDiagnostic {
        source: String,
        severity: String,
        message: String,
        line: usize,
        col: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestion: Option<String>,
    }

    let mut diagnostics = Vec::new();

    for diag in validation_diags {
        let (line, col) = util::offset_to_line_col(source, diag.span.start);
        diagnostics.push(JsonDiagnostic {
            source: "spec".to_string(),
            severity: severity_str(diag.severity),
            message: diag.message.clone(),
            line,
            col,
            id: None,
            suggestion: None,
        });
    }

    for lint in lints {
        let (line, col) = util::offset_to_line_col(source, lint.span.start);
        diagnostics.push(JsonDiagnostic {
            source: "lint".to_string(),
            severity: severity_str(lint.severity),
            message: lint.message.clone(),
            line,
            col,
            id: Some(lint.id.to_string()),
            suggestion: lint.suggestion.clone(),
        });
    }

    let output = JsonOutput {
        file: file.display().to_string(),
        diagnostics,
    };

    if let Ok(json) = serde_json::to_string_pretty(&output) {
        println!("{json}");
    }
}

fn severity_str(s: Severity) -> String {
    match s {
        Severity::Error => "error".to_string(),
        Severity::Warning => "warning".to_string(),
        Severity::Info => "info".to_string(),
    }
}
