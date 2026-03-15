//! Shared helpers for CLI commands: finding .cabal files, loading/parsing,
//! printing diagnostics, and locating component fields in the CST.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use cabalist_parser::diagnostic::Diagnostic;
use cabalist_parser::ParseResult;

/// Find the .cabal file in the given directory. Errors if zero or more than
/// one `.cabal` file is found.
pub fn find_cabal_file(dir: &Path) -> Result<PathBuf> {
    let mut matches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cabal") && path.is_file() {
                matches.push(path);
            }
        }
    }

    match matches.len() {
        0 => bail!("No .cabal file found in {}", dir.display()),
        1 => Ok(matches.into_iter().next().unwrap()),
        n => bail!(
            "Found {n} .cabal files in {}; use --file to specify which one",
            dir.display()
        ),
    }
}

/// Resolve the .cabal file path: use the explicit `--file` path if given,
/// otherwise auto-detect in the current directory.
pub fn resolve_cabal_file(explicit: &Option<PathBuf>) -> Result<PathBuf> {
    match explicit {
        Some(p) => {
            if !p.exists() {
                bail!("File not found: {}", p.display());
            }
            Ok(p.clone())
        }
        None => {
            let cwd = std::env::current_dir().context("Failed to get current directory")?;
            find_cabal_file(&cwd)
        }
    }
}

/// Load a .cabal file from disk and parse it.
pub fn load_and_parse(path: &Path) -> Result<(String, ParseResult)> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let result = cabalist_parser::parse(&source);
    Ok((source, result))
}

/// Convert a byte offset to (line, col) in the source text. Both are 1-based.
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Print a parser diagnostic in GCC-style format: `file:line:col: severity: message`
pub fn print_diagnostic(file: &Path, diag: &Diagnostic, source: &str) {
    use colored::Colorize;

    let (line, col) = offset_to_line_col(source, diag.span.start);
    let severity_str = match diag.severity {
        cabalist_parser::Severity::Error => "error".red().bold().to_string(),
        cabalist_parser::Severity::Warning => "warning".yellow().bold().to_string(),
        cabalist_parser::Severity::Info => "info".cyan().bold().to_string(),
    };
    eprintln!(
        "{}:{}:{}: {}: {}",
        file.display(),
        line,
        col,
        severity_str,
        diag.message
    );
}

/// Print an opinionated lint in GCC-style format.
pub fn print_lint(file: &Path, lint: &cabalist_opinions::Lint, source: &str) {
    use colored::Colorize;

    let (line, col) = offset_to_line_col(source, lint.span.start);
    let severity_str = match lint.severity {
        cabalist_parser::Severity::Error => "error".red().bold().to_string(),
        cabalist_parser::Severity::Warning => "warning".yellow().bold().to_string(),
        cabalist_parser::Severity::Info => "info".cyan().bold().to_string(),
    };
    let id_str = format!("[{}]", lint.id).dimmed().to_string();
    eprintln!(
        "{}:{}:{}: {}: {} {}",
        file.display(),
        line,
        col,
        severity_str,
        lint.message,
        id_str
    );
    if let Some(ref suggestion) = lint.suggestion {
        eprintln!("  {}: {}", "suggestion".green().bold(), suggestion);
    }
}

/// Parse a component string like "library", "exe:my-exe", "test:my-tests"
/// into the (keyword, optional name) pair used by `edit::find_section`.
pub fn parse_component_spec(component: &str) -> (&str, Option<&str>) {
    if component.eq_ignore_ascii_case("library") {
        return ("library", None);
    }
    if let Some(name) = component.strip_prefix("exe:") {
        return ("executable", Some(name));
    }
    if let Some(name) = component.strip_prefix("test:") {
        return ("test-suite", Some(name));
    }
    if let Some(name) = component.strip_prefix("bench:") {
        return ("benchmark", Some(name));
    }
    // If it doesn't match a known prefix, try as a section keyword directly.
    if let Some((keyword, name)) = component.split_once(':') {
        (keyword, Some(name))
    } else {
        (component, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_to_line_col_first_char() {
        assert_eq!(offset_to_line_col("hello\nworld\n", 0), (1, 1));
    }

    #[test]
    fn offset_to_line_col_second_line() {
        assert_eq!(offset_to_line_col("hello\nworld\n", 6), (2, 1));
    }

    #[test]
    fn offset_to_line_col_mid_line() {
        assert_eq!(offset_to_line_col("hello\nworld\n", 8), (2, 3));
    }

    #[test]
    fn offset_to_line_col_past_end() {
        assert_eq!(offset_to_line_col("hi\n", 100), (2, 1));
    }

    #[test]
    fn parse_component_spec_library() {
        assert_eq!(parse_component_spec("library"), ("library", None));
    }

    #[test]
    fn parse_component_spec_exe() {
        assert_eq!(
            parse_component_spec("exe:my-app"),
            ("executable", Some("my-app"))
        );
    }

    #[test]
    fn parse_component_spec_test() {
        assert_eq!(
            parse_component_spec("test:my-tests"),
            ("test-suite", Some("my-tests"))
        );
    }

    #[test]
    fn parse_component_spec_bench() {
        assert_eq!(
            parse_component_spec("bench:my-bench"),
            ("benchmark", Some("my-bench"))
        );
    }

    #[test]
    fn find_cabal_file_no_file() {
        let tmp = std::env::temp_dir().join("cabalist-test-no-cabal");
        let _ = std::fs::create_dir_all(&tmp);
        // Clean out any .cabal files that might exist.
        if let Ok(entries) = std::fs::read_dir(&tmp) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "cabal") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        let result = find_cabal_file(&tmp);
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_cabal_file_one_file() {
        let tmp = std::env::temp_dir().join("cabalist-test-one-cabal");
        let _ = std::fs::create_dir_all(&tmp);
        // Clean out existing cabal files.
        if let Ok(entries) = std::fs::read_dir(&tmp) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "cabal") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        let cabal_path = tmp.join("test.cabal");
        std::fs::write(&cabal_path, "cabal-version: 3.0\n").unwrap();
        let found = find_cabal_file(&tmp).unwrap();
        assert_eq!(found, cabal_path);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
