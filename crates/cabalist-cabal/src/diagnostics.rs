//! Parser for GHC error and warning messages from `cabal build` stderr.
//!
//! GHC's output format is not formally specified, so this is a best-effort parser
//! that handles the most common patterns.

/// A parsed GHC diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhcDiagnostic {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based).
    pub column: u32,
    /// Error or warning.
    pub severity: GhcSeverity,
    /// The main diagnostic message.
    pub message: String,
    /// Warning flag code, e.g. `"-Wunused-imports"`.
    pub code: Option<String>,
    /// Suggestion lines like "Perhaps you meant...".
    pub suggestions: Vec<String>,
}

/// Severity of a GHC diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhcSeverity {
    /// A compilation error.
    Error,
    /// A compiler warning.
    Warning,
}

/// Parse GHC diagnostic messages from stderr output.
///
/// Returns an empty `Vec` if no diagnostics are found or the input is empty.
pub fn parse_diagnostics(stderr: &str) -> Vec<GhcDiagnostic> {
    let mut diagnostics = Vec::new();
    let lines: Vec<&str> = stderr.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(header) = parse_diagnostic_header(lines[i]) {
            // Collect continuation lines (indented lines following the header).
            let mut body_lines = Vec::new();
            i += 1;
            while i < lines.len() && is_continuation_line(lines[i]) {
                body_lines.push(lines[i].trim());
                i += 1;
            }

            let (message, suggestions) = extract_message_and_suggestions(&body_lines);

            diagnostics.push(GhcDiagnostic {
                file: header.file,
                line: header.line,
                column: header.column,
                severity: header.severity,
                message,
                code: header.code,
                suggestions,
            });
        } else {
            i += 1;
        }
    }

    diagnostics
}

/// Parsed info from a diagnostic header line.
struct DiagnosticHeader {
    file: String,
    line: u32,
    column: u32,
    severity: GhcSeverity,
    code: Option<String>,
}

/// Try to parse a line as a GHC diagnostic header.
///
/// Expected formats:
/// - `src/Foo.hs:12:5: error:`
/// - `src/Foo.hs:12:5: warning: [-Wunused-imports]`
/// - `src/Foo.hs:12:5: error: [GHC-12345]`
fn parse_diagnostic_header(line: &str) -> Option<DiagnosticHeader> {
    // Find the pattern: <file>:<line>:<col>: (error|warning)
    // The file path may contain colons on Windows (e.g., C:\foo), so we search
    // for the `: error` or `: warning` suffix pattern and work backwards.

    let (before_severity, severity, after_severity) = if let Some(pos) = line.find(": error") {
        let after = &line[pos + ": error".len()..];
        (
            &line[..pos],
            GhcSeverity::Error,
            after.trim_start_matches(':').trim(),
        )
    } else if let Some(pos) = line.find(": warning") {
        let after = &line[pos + ": warning".len()..];
        (
            &line[..pos],
            GhcSeverity::Warning,
            after.trim_start_matches(':').trim(),
        )
    } else {
        return None;
    };

    // Parse <file>:<line>:<col> from before_severity.
    let (file, line_num, col_num) = parse_file_line_col(before_severity)?;

    // Extract warning/error code from brackets, e.g., [-Wunused-imports] or [GHC-12345]
    let code = if after_severity.starts_with('[') {
        after_severity
            .find(']')
            .map(|end| after_severity[1..end].to_string())
    } else {
        None
    };

    Some(DiagnosticHeader {
        file: file.to_string(),
        line: line_num,
        column: col_num,
        severity,
        code,
    })
}

/// Parse `<file>:<line>:<col>` from the end of a string.
///
/// We parse from the right to handle file paths that might contain colons.
fn parse_file_line_col(s: &str) -> Option<(&str, u32, u32)> {
    // Split from the right: we need at least two colons for :line:col
    let last_colon = s.rfind(':')?;
    let col_str = &s[last_colon + 1..];
    let rest = &s[..last_colon];

    let second_last_colon = rest.rfind(':')?;
    let line_str = &rest[second_last_colon + 1..];
    let file = &rest[..second_last_colon];

    let line_num = line_str.parse::<u32>().ok()?;
    let col_num = col_str.parse::<u32>().ok()?;

    if file.is_empty() {
        return None;
    }

    Some((file, line_num, col_num))
}

/// Check if a line is a continuation of a diagnostic (indented with spaces).
fn is_continuation_line(line: &str) -> bool {
    if line.is_empty() {
        // Empty lines within a diagnostic block can occur, but we stop at them
        // to avoid collecting unrelated output.
        return false;
    }
    // Continuation lines are indented with spaces.
    line.starts_with(' ') || line.starts_with('\t')
}

/// Extract the main message and suggestion lines from diagnostic body lines.
fn extract_message_and_suggestions(lines: &[&str]) -> (String, Vec<String>) {
    let mut message_parts = Vec::new();
    let mut suggestions = Vec::new();

    for &line in lines {
        if is_suggestion_line(line) {
            suggestions.push(line.to_string());
        } else {
            message_parts.push(line);
        }
    }

    let message = message_parts.join("\n");
    (message, suggestions)
}

/// Check if a line is a suggestion (e.g., "Perhaps you meant...", "Suggested fix:").
fn is_suggestion_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.starts_with("perhaps")
        || lower.starts_with("suggested fix")
        || lower.starts_with("use ")
        || lower.starts_with("did you mean")
        || lower.contains("perhaps you meant")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_error() {
        let stderr = "src/Foo.hs:12:5: error:\n    Not in scope: 'bar'\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "src/Foo.hs");
        assert_eq!(diags[0].line, 12);
        assert_eq!(diags[0].column, 5);
        assert_eq!(diags[0].severity, GhcSeverity::Error);
        assert_eq!(diags[0].message, "Not in scope: 'bar'");
        assert!(diags[0].code.is_none());
        assert!(diags[0].suggestions.is_empty());
    }

    #[test]
    fn parse_single_warning_with_code() {
        let stderr =
            "src/Bar.hs:3:1: warning: [-Wunused-imports]\n    The import of 'Data.List' is redundant\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "src/Bar.hs");
        assert_eq!(diags[0].line, 3);
        assert_eq!(diags[0].column, 1);
        assert_eq!(diags[0].severity, GhcSeverity::Warning);
        assert_eq!(diags[0].message, "The import of 'Data.List' is redundant");
        assert_eq!(diags[0].code.as_deref(), Some("-Wunused-imports"));
    }

    #[test]
    fn parse_multiple_diagnostics() {
        let stderr = "\
Building library for my-project-0.1.0.0...
[1 of 3] Compiling Data.Foo
src/Data/Foo.hs:10:1: warning: [-Wunused-imports]
    The import of 'Data.Map' is redundant
[2 of 3] Compiling Data.Bar
src/Data/Bar.hs:25:5: error:
    Not in scope: 'undefined'
    Perhaps you meant 'underfined' (line 20)
";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 2);

        assert_eq!(diags[0].file, "src/Data/Foo.hs");
        assert_eq!(diags[0].severity, GhcSeverity::Warning);
        assert_eq!(diags[0].code.as_deref(), Some("-Wunused-imports"));

        assert_eq!(diags[1].file, "src/Data/Bar.hs");
        assert_eq!(diags[1].severity, GhcSeverity::Error);
        assert_eq!(diags[1].line, 25);
        assert_eq!(diags[1].column, 5);
        assert_eq!(diags[1].suggestions.len(), 1);
        assert!(diags[1].suggestions[0].contains("Perhaps you meant"));
    }

    #[test]
    fn parse_diagnostic_with_suggestions() {
        let stderr = "\
src/Foo.hs:12:5: error:
    Not in scope: 'bar'
    Perhaps you meant 'baz' (imported from Data.Map)
    Perhaps you meant 'bat' (line 5)
";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Not in scope: 'bar'");
        assert_eq!(diags[0].suggestions.len(), 2);
    }

    #[test]
    fn parse_empty_input() {
        let diags = parse_diagnostics("");
        assert!(diags.is_empty());
    }

    #[test]
    fn parse_non_diagnostic_output() {
        let stderr = "\
Resolving dependencies...
Build profile: -w ghc-9.8.2 -O1
In order, the following will be built:
 - my-project-0.1.0.0 (lib) (first run)
Configuring library for my-project-0.1.0.0...
Preprocessing library for my-project-0.1.0.0...
Building library for my-project-0.1.0.0...
";
        let diags = parse_diagnostics(stderr);
        assert!(diags.is_empty());
    }

    #[test]
    fn parse_multi_line_error_message() {
        let stderr = "\
src/Foo.hs:10:5: error:
    Couldn't match expected type 'Int'
                with actual type 'String'
    In the expression: \"hello\"
    In an equation for 'x': x = \"hello\"
";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Couldn't match expected type"));
        assert!(diags[0].message.contains("actual type"));
    }

    #[test]
    fn parse_windows_style_path() {
        let stderr = "C:\\Users\\dev\\src\\Foo.hs:5:3: error:\n    Some error\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "C:\\Users\\dev\\src\\Foo.hs");
        assert_eq!(diags[0].line, 5);
        assert_eq!(diags[0].column, 3);
    }

    #[test]
    fn parse_ghc_error_code_format() {
        let stderr = "src/Foo.hs:1:1: error: [GHC-88464]\n    Module 'Foo' does not export 'bar'\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("GHC-88464"));
    }
}
