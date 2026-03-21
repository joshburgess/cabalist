//! Document formatting via the LSP `textDocument/formatting` request.
//!
//! Performs a round-trip parse → render to normalize whitespace, and optionally
//! sorts dependencies and modules based on project configuration.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// Format a `.cabal` source file and return the edits needed to transform
/// the original source into the formatted version.
///
/// Returns an empty vec if no changes are needed (already formatted).
pub fn format_document(
    source: &str,
    line_index: &LineIndex,
    project_root: &std::path::Path,
) -> Vec<TextEdit> {
    let config = cabalist_opinions::config::find_and_load_config(project_root);

    let mut current = source.to_string();

    // Sort dependencies if configured.
    if config.formatting.sort_dependencies {
        current = cabalist_opinions::fmt::sort_list_field(&current, "build-depends");
    }

    // Sort modules if configured.
    if config.formatting.sort_modules {
        current = cabalist_opinions::fmt::sort_list_field(&current, "exposed-modules");
        current = cabalist_opinions::fmt::sort_list_field(&current, "other-modules");
    }

    // Round-trip through the parser to normalize.
    let result = cabalist_parser::parse(&current);
    let formatted = result.cst.render();

    if formatted == source {
        return Vec::new();
    }

    // Return a single edit that replaces the entire document.
    let end_pos = line_index.offset_to_position(source.len());
    vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: end_pos,
        },
        new_text: formatted,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_already_clean() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n";
        let line_index = LineIndex::new(source);
        let edits = format_document(source, &line_index, std::path::Path::new("."));
        assert!(edits.is_empty(), "clean file should produce no edits");
    }

    #[test]
    fn format_produces_valid_range() {
        let source = "cabal-version: 3.0\nname:    test\nversion: 0.1\n";
        let line_index = LineIndex::new(source);
        let edits = format_document(source, &line_index, std::path::Path::new("."));
        // Even if no format changes (round-trip is clean), edits should be valid.
        for edit in &edits {
            assert!(edit.range.start.line <= edit.range.end.line);
        }
    }
}
