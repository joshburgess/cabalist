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
pub fn format_document(source: &str, line_index: &LineIndex) -> Vec<TextEdit> {
    let project_root = std::env::current_dir().unwrap_or_default();
    let config = cabalist_opinions::config::find_and_load_config(&project_root);

    let mut current = source.to_string();

    // Sort dependencies if configured.
    if config.formatting.sort_dependencies {
        current = sort_list_field(&current, "build-depends");
    }

    // Sort modules if configured.
    if config.formatting.sort_modules {
        current = sort_list_field(&current, "exposed-modules");
        current = sort_list_field(&current, "other-modules");
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

/// Sort items within all instances of a list field.
///
/// This is a simplified version of the CLI's sort logic. It collects items,
/// sorts them, and rebuilds the field value.
fn sort_list_field(source: &str, field_name: &str) -> String {
    use cabalist_parser::edit::{self, EditBatch};

    let section_keys = collect_section_keys(source);
    let mut current = source.to_string();

    for key in &section_keys {
        let Some((result, _section_id, field_id)) =
            find_section_field(&current, key, field_name)
        else {
            continue;
        };

        let items = extract_list_items(&result.cst, field_id);
        if items.len() <= 1 {
            continue;
        }

        let mut sorted = items.clone();
        sorted.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        if items == sorted {
            continue;
        }

        // Remove in reverse, re-add in sorted order.
        for item in items.iter().rev() {
            let Some((re_result, _, re_field)) =
                find_section_field(&current, key, field_name)
            else {
                break;
            };
            let item_name = item.split_whitespace().next().unwrap_or(item);
            let edits = edit::remove_list_item(&re_result.cst, re_field, item_name);
            if !edits.is_empty() {
                let mut batch = EditBatch::new();
                batch.add_all(edits);
                current = batch.apply(&re_result.cst.source);
            }
        }

        for item in &sorted {
            let Some((re_result, _, re_field)) =
                find_section_field(&current, key, field_name)
            else {
                break;
            };
            let edits = edit::add_list_item(&re_result.cst, re_field, item, false);
            if !edits.is_empty() {
                let mut batch = EditBatch::new();
                batch.add_all(edits);
                current = batch.apply(&re_result.cst.source);
            }
        }
    }

    current
}

#[derive(Debug, Clone)]
struct SectionKey {
    keyword: String,
    name: Option<String>,
}

fn collect_section_keys(source: &str) -> Vec<SectionKey> {
    use cabalist_parser::ast::{derive_ast, Component};

    let result = cabalist_parser::parse(source);
    let ast = derive_ast(&result.cst);
    let mut keys = Vec::new();

    for comp in ast.all_components() {
        let key = match comp {
            Component::Library(lib) => SectionKey {
                keyword: "library".to_string(),
                name: lib.fields.name.map(|s| s.to_string()),
            },
            Component::Executable(exe) => SectionKey {
                keyword: "executable".to_string(),
                name: exe.fields.name.map(|s| s.to_string()),
            },
            Component::TestSuite(ts) => SectionKey {
                keyword: "test-suite".to_string(),
                name: ts.fields.name.map(|s| s.to_string()),
            },
            Component::Benchmark(bm) => SectionKey {
                keyword: "benchmark".to_string(),
                name: bm.fields.name.map(|s| s.to_string()),
            },
        };
        keys.push(key);
    }
    for cs in &ast.common_stanzas {
        keys.push(SectionKey {
            keyword: "common".to_string(),
            name: Some(cs.name.to_string()),
        });
    }
    keys
}

fn find_section_field(
    source: &str,
    key: &SectionKey,
    field_name: &str,
) -> Option<(
    cabalist_parser::ParseResult,
    cabalist_parser::span::NodeId,
    cabalist_parser::span::NodeId,
)> {
    use cabalist_parser::edit;

    let result = cabalist_parser::parse(source);
    let section_id = edit::find_section(&result.cst, &key.keyword, key.name.as_deref())?;
    let field_id = edit::find_field(&result.cst, section_id, field_name)?;
    Some((result, section_id, field_id))
}

fn extract_list_items(
    cst: &cabalist_parser::cst::CabalCst,
    field_node: cabalist_parser::span::NodeId,
) -> Vec<String> {
    use cabalist_parser::cst::CstNodeKind;

    let node = &cst.nodes[field_node.0];
    let mut items = Vec::new();

    for &child_id in &node.children {
        let child = &cst.nodes[child_id.0];
        if matches!(child.kind, CstNodeKind::ValueLine) {
            let text = &cst.source[child.span.start..child.span.end];
            let text = text.trim();
            let text = text.trim_start_matches(',').trim_end_matches(',').trim();
            if !text.is_empty() {
                items.push(text.to_string());
            }
        }
    }

    if items.is_empty() {
        if let Some(ref val) = node.field_value {
            let text = &cst.source[val.start..val.end];
            let text = text.trim();
            for part in text.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_already_clean() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n";
        let line_index = LineIndex::new(source);
        let edits = format_document(source, &line_index);
        assert!(edits.is_empty(), "clean file should produce no edits");
    }

    #[test]
    fn format_produces_valid_range() {
        let source = "cabal-version: 3.0\nname:    test\nversion: 0.1\n";
        let line_index = LineIndex::new(source);
        let edits = format_document(source, &line_index);
        // Even if no format changes (round-trip is clean), edits should be valid.
        for edit in &edits {
            assert!(edit.range.start.line <= edit.range.end.line);
        }
    }
}
