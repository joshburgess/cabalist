//! Shared formatting logic for `.cabal` files.
//!
//! Provides field-level sorting (dependencies, modules) used by the CLI `fmt`
//! command, the TUI `Ctrl+F`, and the LSP formatting handler.

use cabalist_parser::ast::{derive_ast, Component};
use cabalist_parser::edit::{self, EditBatch};

/// Sort items within all instances of a specific list field across all sections.
///
/// For each section containing the target field, reads items, sorts them
/// alphabetically, and rewrites by removing items one-at-a-time (re-parsing
/// between each removal) then re-adding in sorted order.
pub fn sort_list_field(source: &str, field_name: &str) -> String {
    let section_keys = collect_section_keys(source);
    let mut current = source.to_string();

    for key in &section_keys {
        let Some((result, _section_id, field_id)) = find_section_field(&current, key, field_name)
        else {
            continue;
        };

        let items = extract_list_items(&result.cst, field_id);
        if items.len() <= 1 {
            continue;
        }

        let mut sorted = items.clone();
        sorted.sort_by_key(|a| a.to_ascii_lowercase());
        if items == sorted {
            continue;
        }

        // Detect original list style (multiline vs single-line) before editing.
        let style = edit::detect_list_style(&result.cst, field_id);
        let use_sort = !matches!(style, edit::ListStyle::SingleLine);

        // Remove items in reverse order, re-parsing between each removal.
        for item in items.iter().rev() {
            let Some((re_result, _, re_field)) = find_section_field(&current, key, field_name)
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

        // Re-add items in sorted order, preserving the original layout style.
        for item in &sorted {
            let Some((re_result, _, re_field)) = find_section_field(&current, key, field_name)
            else {
                break;
            };
            let edits = edit::add_list_item(&re_result.cst, re_field, item, use_sort);
            if !edits.is_empty() {
                let mut batch = EditBatch::new();
                batch.add_all(edits);
                current = batch.apply(&re_result.cst.source);
            }
        }
    }

    current
}

/// A section identifier that survives re-parsing: keyword + optional name.
#[derive(Debug, Clone)]
struct SectionKey {
    keyword: String,
    name: Option<String>,
}

/// Collect stable section identifiers from the AST.
fn collect_section_keys(source: &str) -> Vec<SectionKey> {
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

/// Re-find a section and field in a freshly parsed source.
fn find_section_field(
    source: &str,
    key: &SectionKey,
    field_name: &str,
) -> Option<(
    cabalist_parser::ParseResult,
    cabalist_parser::span::NodeId,
    cabalist_parser::span::NodeId,
)> {
    let result = cabalist_parser::parse(source);
    let section_id = edit::find_section(&result.cst, &key.keyword, key.name.as_deref())?;
    let field_id = edit::find_field(&result.cst, section_id, field_name)?;
    Some((result, section_id, field_id))
}

/// Extract individual items from a list field's value.
pub fn extract_list_items(
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
            let text = text
                .trim()
                .trim_start_matches(',')
                .trim_end_matches(',')
                .trim();
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
