//! `cabalist-cli fmt` — Format the .cabal file.
//!
//! Performs round-trip formatting (parse + render) and optionally sorts
//! dependencies and modules alphabetically.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_opinions::config::find_and_load_config;
use cabalist_parser::ast::derive_ast;
use cabalist_parser::edit::{self, EditBatch};

use crate::util;

pub fn run(file: &Option<PathBuf>, check: bool) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (original_source, _result) = util::load_and_parse(&cabal_path)?;

    // Load config for formatting preferences.
    let project_root = cabal_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = find_and_load_config(project_root);

    let mut current_source = original_source.clone();

    // Sort dependencies if configured.
    if config.formatting.sort_dependencies {
        current_source = sort_list_field(&current_source, "build-depends");
    }

    // Sort modules if configured.
    if config.formatting.sort_modules {
        current_source = sort_list_field(&current_source, "exposed-modules");
        current_source = sort_list_field(&current_source, "other-modules");
    }

    // Re-parse and render to normalize (the round-trip should be clean).
    let final_result = cabalist_parser::parse(&current_source);
    let formatted = final_result.cst.render();

    if check {
        if formatted != original_source {
            eprintln!("{} needs formatting", cabal_path.display());
            return Ok(ExitCode::from(1));
        }
        println!("{} is correctly formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    if formatted == original_source {
        println!("{} is already formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    std::fs::write(&cabal_path, &formatted)?;
    println!("Formatted {}", cabal_path.display());
    Ok(ExitCode::SUCCESS)
}

/// Sort items within all instances of a specific list field across all sections.
///
/// Strategy: for each section containing the target field, read the items,
/// sort them, remove all, and re-add in sorted order — preserving the
/// original list style.
fn sort_list_field(source: &str, field_name: &str) -> String {
    let result = cabalist_parser::parse(source);
    let cst = &result.cst;
    let ast = derive_ast(cst);

    // Collect all section node IDs that might contain the target field.
    let mut section_ids = Vec::new();
    for comp in ast.all_components() {
        section_ids.push(comp.cst_node());
    }
    for cs in &ast.common_stanzas {
        section_ids.push(cs.fields.cst_node);
    }

    let mut current = source.to_string();

    for section_id in &section_ids {
        // Re-parse each iteration since edits shift offsets.
        let re_result = cabalist_parser::parse(&current);
        let re_cst = &re_result.cst;

        // We need to re-find the section after re-parsing. Use the AST to
        // locate sections by matching keyword + name from the original AST.
        // Simpler approach: find the field directly in this section.
        let Some(field_id) = edit::find_field(re_cst, *section_id, field_name) else {
            continue;
        };

        // Detect the current list style.
        let style = edit::detect_list_style(re_cst, field_id);

        // Extract current items from the field value.
        let items = extract_list_items(re_cst, field_id);
        if items.len() <= 1 {
            continue; // Nothing to sort.
        }

        // Check if already sorted.
        let mut sorted_items = items.clone();
        sorted_items.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        if items == sorted_items {
            continue; // Already sorted.
        }

        // Remove all items, then re-add in sorted order.
        // We do this by removing each item and then adding them back sorted.
        let mut batch = EditBatch::new();

        // Remove all items.
        for item in items.iter().rev() {
            // Extract just the package name (before any version constraint) for matching.
            let item_name = item.split_whitespace().next().unwrap_or(item);
            let edits = edit::remove_list_item(re_cst, field_id, item_name);
            batch.add_all(edits);
        }

        // Apply removals first.
        if batch.is_empty() {
            continue;
        }
        let after_removal = batch.apply(&re_cst.source);

        // Re-parse after removal, re-find the field, and add items back sorted.
        let re2 = cabalist_parser::parse(&after_removal);
        let re2_cst = &re2.cst;
        let Some(field_id2) = edit::find_field(re2_cst, *section_id, field_name) else {
            // If we can't find the field after removal, something went wrong.
            // Fall back to the pre-removal state.
            continue;
        };

        let mut batch2 = EditBatch::new();
        // Add items back in sorted order (sort=false since we pre-sorted).
        for item in &sorted_items {
            let edits = edit::add_list_item(re2_cst, field_id2, item, false);
            batch2.add_all(edits);
        }

        if !batch2.is_empty() {
            current = batch2.apply(&re2_cst.source);
        } else {
            current = after_removal;
        }

        let _ = style; // We preserve the original style via the edit engine.
    }

    current
}

/// Extract the individual items from a list field's value.
fn extract_list_items(
    cst: &cabalist_parser::cst::CabalCst,
    field_node: cabalist_parser::span::NodeId,
) -> Vec<String> {
    use cabalist_parser::cst::CstNodeKind;

    let node = &cst.nodes[field_node.0];
    let mut items = Vec::new();

    // Collect value lines from children.
    for &child_id in &node.children {
        let child = &cst.nodes[child_id.0];
        if matches!(child.kind, CstNodeKind::ValueLine) {
            let text = &cst.source[child.span.start..child.span.end];
            let text = text.trim();
            // Strip leading/trailing commas.
            let text = text.trim_start_matches(',').trim_end_matches(',').trim();
            if !text.is_empty() {
                items.push(text.to_string());
            }
        }
    }

    // If no ValueLine children, try the field_value directly.
    if items.is_empty() {
        if let Some(ref val) = node.field_value {
            let text = &cst.source[val.start..val.end];
            let text = text.trim();
            // Split comma-separated single-line values.
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
