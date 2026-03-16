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
pub fn lint_to_lsp(lint: &Lint, line_index: &LineIndex) -> lsp_types::Diagnostic {
    let mut message = lint.message.clone();
    if let Some(ref suggestion) = lint.suggestion {
        message.push_str("\nSuggestion: ");
        message.push_str(suggestion);
    }

    lsp_types::Diagnostic {
        range: line_index.span_to_range(lint.span),
        severity: Some(convert_severity(lint.severity)),
        source: Some("cabalist".into()),
        code: Some(NumberOrString::String(lint.id.to_string())),
        message,
        ..Default::default()
    }
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
