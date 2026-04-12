//! Rename provider for `.cabal` files.
//!
//! Supports renaming common stanza names and updating all `import:` references.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// Compute the rename edits for renaming the symbol at the given position.
///
/// Currently supports renaming `common` stanza names (at the definition or
/// at any `import:` reference), updating all occurrences.
pub fn prepare_rename(
    source: &str,
    line_index: &LineIndex,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let stanza_name = find_stanza_name_at(source, line_index, position)?;

    // Find the word range at the cursor position.
    let offset = line_index.position_to_offset(position);
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_text = &source[line_start..];
    let trimmed = line_text.trim();

    // Find the stanza name within the line.
    if let Some(name_start) = trimmed.find(&stanza_name) {
        let indent = line_text.len() - trimmed.len();
        let abs_start = line_start + indent + name_start;
        let abs_end = abs_start + stanza_name.len();
        let range = line_index.span_to_range(cabalist_parser::span::Span::new(abs_start, abs_end));
        Some(PrepareRenameResponse::Range(range))
    } else {
        None
    }
}

/// Compute workspace edits for renaming a common stanza.
pub fn rename(
    source: &str,
    line_index: &LineIndex,
    uri: &Url,
    position: Position,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let old_name = find_stanza_name_at(source, line_index, position)?;

    let mut edits = Vec::new();

    // Find all occurrences: `common <name>` definitions and `import: <name>` references.
    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // Check for `common <old_name>` definition.
        if lower.starts_with("common ") {
            let name_part = trimmed["common ".len()..].trim();
            if name_part.eq_ignore_ascii_case(&old_name) {
                let indent = line.len() - trimmed.len();
                // Find exact position of the name.
                if let Some(pos) = trimmed.find(name_part) {
                    let col = indent + pos;
                    edits.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: line_num as u32,
                                character: col as u32,
                            },
                            end: Position {
                                line: line_num as u32,
                                character: (col + name_part.len()) as u32,
                            },
                        },
                        new_text: new_name.to_string(),
                    });
                }
            }
        }

        // Check for `import: <old_name>` reference.
        if lower.starts_with("import:") {
            let value = trimmed["import:".len()..].trim();
            if value.eq_ignore_ascii_case(&old_name) {
                let indent = line.len() - trimmed.len();
                if let Some(pos) = trimmed.rfind(value) {
                    let col = indent + pos;
                    edits.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: line_num as u32,
                                character: col as u32,
                            },
                            end: Position {
                                line: line_num as u32,
                                character: (col + value.len()) as u32,
                            },
                        },
                        new_text: new_name.to_string(),
                    });
                }
            }
        }
    }

    if edits.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}

/// Find the common stanza name referenced at the given position.
///
/// Works on both `common <name>` definitions and `import: <name>` references.
fn find_stanza_name_at(source: &str, line_index: &LineIndex, position: Position) -> Option<String> {
    let offset = line_index.position_to_offset(position);
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    let line_text = &source[line_start..line_end];
    let trimmed = line_text.trim();
    let lower = trimmed.to_lowercase();

    if lower.starts_with("common ") {
        let name = trimmed["common ".len()..].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    if lower.starts_with("import:") {
        let name = trimmed["import:".len()..].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_source() -> &'static str {
        "\
cabal-version: 3.0
name: test
version: 0.1

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules: Lib

executable my-app
  import: warnings
  main-is: Main.hs
"
    }

    #[test]
    fn prepare_rename_on_common_definition() {
        let source = test_source();
        let li = LineIndex::new(source);
        let pos = Position {
            line: 4,
            character: 10,
        }; // on "warnings" in "common warnings"
        let result = prepare_rename(source, &li, pos);
        assert!(result.is_some());
    }

    #[test]
    fn prepare_rename_on_import_reference() {
        let source = test_source();
        let li = LineIndex::new(source);
        let pos = Position {
            line: 8,
            character: 12,
        }; // on "warnings" in "import: warnings"
        let result = prepare_rename(source, &li, pos);
        assert!(result.is_some());
    }

    #[test]
    fn prepare_rename_on_unrelated_line() {
        let source = test_source();
        let li = LineIndex::new(source);
        let pos = Position {
            line: 1,
            character: 3,
        }; // on "name: test"
        let result = prepare_rename(source, &li, pos);
        assert!(result.is_none());
    }

    #[test]
    fn rename_updates_definition_and_all_imports() {
        let source = test_source();
        let li = LineIndex::new(source);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let pos = Position {
            line: 4,
            character: 10,
        };

        let result = rename(source, &li, &uri, pos, "shared-opts");
        assert!(result.is_some());
        let ws_edit = result.unwrap();
        let changes = ws_edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();

        // Should have 3 edits: 1 definition + 2 import references.
        assert_eq!(edits.len(), 3, "should rename definition + 2 imports");
        for edit in edits {
            assert_eq!(edit.new_text, "shared-opts");
        }
    }

    #[test]
    fn rename_from_import_also_works() {
        let source = test_source();
        let li = LineIndex::new(source);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let pos = Position {
            line: 8,
            character: 12,
        }; // on import: warnings

        let result = rename(source, &li, &uri, pos, "common-settings");
        assert!(result.is_some());
        let ws_edit = result.unwrap();
        let changes = ws_edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 3);
    }
}
