//! Diagnostic conversion: parser, validation, and opinion diagnostics → LSP format.

use cabalist_parser::diagnostic::{Diagnostic, Severity};
use cabalist_opinions::Lint;
use tower_lsp::lsp_types::{self, DiagnosticSeverity, NumberOrString};

use crate::convert::LineIndex;

/// Convert a parser/validation `Diagnostic` to an LSP diagnostic.
pub fn parser_diagnostic_to_lsp(
    diag: &Diagnostic,
    line_index: &LineIndex,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: line_index.span_to_range(diag.span),
        severity: Some(convert_severity(diag.severity)),
        source: Some("cabalist".into()),
        message: diag.message.clone(),
        ..Default::default()
    }
}

/// Convert an opinion `Lint` to an LSP diagnostic.
///
/// Attaches structured data as JSON in the `data` field so that code actions
/// can extract package names and versions without parsing the human-readable
/// message string.
pub fn lint_to_lsp(lint: &Lint, line_index: &LineIndex) -> lsp_types::Diagnostic {
    let mut message = lint.message.clone();
    if let Some(ref suggestion) = lint.suggestion {
        message.push_str("\nSuggestion: ");
        message.push_str(suggestion);
    }

    // Extract structured data from the lint for code actions.
    let data = extract_lint_data(lint);

    lsp_types::Diagnostic {
        range: line_index.span_to_range(lint.span),
        severity: Some(convert_severity(lint.severity)),
        source: Some("cabalist".into()),
        code: Some(NumberOrString::String(lint.id.to_string())),
        message,
        data,
        ..Default::default()
    }
}

/// Extract structured metadata from a lint message for use by code actions.
///
/// Returns a JSON value with relevant fields (e.g., `{"package": "text"}`)
/// so code actions don't need to parse the human-readable message.
fn extract_lint_data(lint: &Lint) -> Option<serde_json::Value> {
    match lint.id {
        "missing-upper-bound" | "missing-lower-bound" | "wide-any-version" | "duplicate-dep" => {
            // These lints are about a specific dependency.
            // Extract the package name from: "Dependency 'pkg' ..."
            let pkg = extract_quoted(&lint.message);
            // Extract version info from: "... (>=X.Y)" or "(^>=X.Y)"
            let version = lint.message
                .find('(')
                .and_then(|start| {
                    let rest = &lint.message[start + 1..];
                    rest.find(')').map(|end| rest[..end].trim().to_string())
                });

            let mut map = serde_json::Map::new();
            if let Some(p) = pkg {
                map.insert("package".into(), serde_json::Value::String(p));
            }
            if let Some(v) = version {
                map.insert("constraint".into(), serde_json::Value::String(v));
            }
            Some(serde_json::Value::Object(map))
        }
        "unused-flag" => {
            let name = extract_quoted(&lint.message);
            name.map(|n| serde_json::json!({"flag": n}))
        }
        _ => None,
    }
}

/// Extract a single-quoted string from a message.
fn extract_quoted(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Run the full diagnostic pipeline on a document and return LSP diagnostics.
pub fn compute_diagnostics(
    source: &str,
    line_index: &LineIndex,
    project_root: &std::path::Path,
) -> Vec<lsp_types::Diagnostic> {
    let result = cabalist_parser::parse(source);
    let mut lsp_diags = Vec::new();

    // 1. Parser diagnostics.
    for diag in &result.diagnostics {
        lsp_diags.push(parser_diagnostic_to_lsp(diag, line_index));
    }

    // 2. Spec validation diagnostics.
    let validation_diags = cabalist_parser::validate(&result.cst);
    for diag in &validation_diags {
        lsp_diags.push(parser_diagnostic_to_lsp(diag, line_index));
    }

    // 3. Opinionated lints (including filesystem-aware lints).
    let ast = cabalist_parser::ast::derive_ast(&result.cst);
    let config = cabalist_opinions::config::find_and_load_config(project_root);
    let lint_config = config.lints.to_lint_config();
    let lints = cabalist_opinions::run_all_lints(&ast, &lint_config, project_root);
    for lint in &lints {
        lsp_diags.push(lint_to_lsp(lint, line_index));
    }

    lsp_diags
}

fn convert_severity(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::LineIndex;

    #[test]
    fn parser_diag_converts() {
        let source = "name: foo\n";
        let line_index = LineIndex::new(source);
        let diag = Diagnostic::warning(
            cabalist_parser::span::Span::new(0, 4),
            "test warning".to_string(),
        );
        let lsp = parser_diagnostic_to_lsp(&diag, &line_index);
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(lsp.source.as_deref(), Some("cabalist"));
        assert_eq!(lsp.message, "test warning");
    }

    #[test]
    fn lint_diag_includes_id_and_suggestion() {
        let source = "name: foo\n";
        let line_index = LineIndex::new(source);
        let lint = Lint {
            id: "test-lint",
            severity: Severity::Info,
            message: "Some issue".to_string(),
            span: cabalist_parser::span::Span::new(0, 4),
            suggestion: Some("Fix it".to_string()),
        };
        let lsp = lint_to_lsp(&lint, &line_index);
        assert_eq!(lsp.code, Some(NumberOrString::String("test-lint".into())));
        assert!(lsp.message.contains("Fix it"));
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::INFORMATION));
    }

    #[test]
    fn compute_diagnostics_on_minimal_file() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n";
        let line_index = LineIndex::new(source);
        let tmp = std::env::temp_dir();
        let diags = compute_diagnostics(source, &line_index, &tmp);
        // Should have some info-level lints (missing-synopsis, etc.) but no errors.
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(errors.is_empty(), "minimal valid file should have no errors");
    }
}
