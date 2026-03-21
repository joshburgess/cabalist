//! Go-to-definition provider for `.cabal` files.
//!
//! Supports jumping from `import: stanza-name` to the `common stanza-name`
//! section definition.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// Compute the go-to-definition target for the given position.
///
/// Currently supports:
/// - `import: <name>` -> jumps to `common <name>` section
pub fn goto_definition(
    source: &str,
    line_index: &LineIndex,
    position: Position,
) -> Option<Location> {
    let offset = line_index.position_to_offset(position);

    // Find the current line.
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    let line_text = &source[line_start..line_end];
    let trimmed = line_text.trim();

    // Check if we're on an `import:` field.
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("import:") {
        return None;
    }

    let stanza_name = trimmed["import:".len()..].trim();
    if stanza_name.is_empty() {
        return None;
    }

    // Find the `common <name>` section in the source.
    let target_pattern = format!("common {stanza_name}");
    for (line_num, line) in source.lines().enumerate() {
        let ltrimmed = line.trim();
        if ltrimmed.eq_ignore_ascii_case(&target_pattern)
            || ltrimmed
                .to_lowercase()
                .starts_with(&format!("common {}", stanza_name.to_lowercase()))
        {
            let target_pos = Position {
                line: line_num as u32,
                character: 0,
            };
            let target_range = Range {
                start: target_pos,
                end: Position {
                    line: line_num as u32,
                    character: ltrimmed.len() as u32,
                },
            };
            // We don't have the URI here — the caller will supply it.
            // Return a Location with a placeholder URI that the server will fill.
            return Some(Location {
                uri: Url::parse("file:///placeholder").unwrap(),
                range: target_range,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_from_import_to_common_stanza() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules: Lib
";
        let line_index = LineIndex::new(source);
        // Position on the "import: warnings" line (line 8, char 10 = on "warnings")
        let pos = Position {
            line: 8,
            character: 10,
        };
        let result = goto_definition(source, &line_index, pos);
        assert!(result.is_some());
        let loc = result.unwrap();
        assert_eq!(loc.range.start.line, 4); // "common warnings" is on line 4
    }

    #[test]
    fn no_definition_for_non_import_line() {
        let source = "cabal-version: 3.0\nname: test\n";
        let line_index = LineIndex::new(source);
        let pos = Position {
            line: 0,
            character: 5,
        };
        let result = goto_definition(source, &line_index, pos);
        assert!(result.is_none());
    }

    #[test]
    fn no_definition_for_missing_stanza() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  import: nonexistent
  exposed-modules: Lib
";
        let line_index = LineIndex::new(source);
        let pos = Position {
            line: 5,
            character: 10,
        };
        let result = goto_definition(source, &line_index, pos);
        assert!(result.is_none());
    }
}
