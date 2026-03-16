//! Edit engine for surgical CST mutations that preserve formatting.
//!
//! Edits work by producing [`TextEdit`]s (insertions/deletions at byte offsets)
//! that are collected in an [`EditBatch`] and applied atomically to the source
//! string. After applying, the caller re-parses from scratch to get a fresh
//! CST + AST.
//!
//! The key challenge is list field editing: `.cabal` files use several different
//! formatting styles for list fields (single-line, leading-comma, trailing-comma,
//! no-comma), and the edit engine must detect and follow the existing style.

use crate::cst::{CabalCst, CstNodeKind};
use crate::span::{NodeId, Span};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The detected formatting style of a list field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    /// Single line, comma-separated: `build-depends: base, text, aeson`
    SingleLine,
    /// Multi-line, leading comma: each continuation starts with `, `
    LeadingComma,
    /// Multi-line, trailing comma: each line ends with `,`
    TrailingComma,
    /// Multi-line, no commas (whitespace-separated, e.g. module lists)
    NoComma,
}

/// A text edit to apply to the source.
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// Byte range to replace (can be empty for pure insertions).
    pub range: Span,
    /// Replacement text.
    pub replacement: String,
}

/// A batch of edits to apply atomically.
#[derive(Debug, Clone)]
pub struct EditBatch {
    edits: Vec<TextEdit>,
}

impl Default for EditBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl EditBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    /// Add an edit to the batch.
    pub fn add(&mut self, edit: TextEdit) {
        self.edits.push(edit);
    }

    /// Add multiple edits to the batch.
    pub fn add_all(&mut self, edits: Vec<TextEdit>) {
        self.edits.extend(edits);
    }

    /// Whether the batch contains any edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Apply all edits to the source string, returning the new source.
    ///
    /// Edits are sorted by position and applied from end to start so earlier
    /// offsets remain valid. Panics if any edits overlap.
    pub fn apply(mut self, source: &str) -> String {
        if self.edits.is_empty() {
            return source.to_owned();
        }

        // Sort by start descending so we apply from end to start.
        self.edits.sort_by(|a, b| b.range.start.cmp(&a.range.start));

        // Verify no overlapping edits.
        for pair in self.edits.windows(2) {
            // pair[0] has higher start than pair[1] (descending order).
            assert!(
                pair[0].range.start >= pair[1].range.end,
                "overlapping edits: {:?} and {:?}",
                pair[1].range,
                pair[0].range,
            );
        }

        let mut result = source.to_owned();
        for edit in &self.edits {
            result.replace_range(edit.range.start..edit.range.end, &edit.replacement);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// List style detection
// ---------------------------------------------------------------------------

/// Detect the list style of a field node.
///
/// Examines the field's inline value and `ValueLine` children to determine
/// which formatting convention is used.
pub fn detect_list_style(cst: &CabalCst, field_node: NodeId) -> ListStyle {
    let node = cst.node(field_node);
    debug_assert_eq!(node.kind, CstNodeKind::Field);

    let value_lines: Vec<NodeId> = node
        .children
        .iter()
        .copied()
        .filter(|&id| cst.node(id).kind == CstNodeKind::ValueLine)
        .collect();

    // If there are no continuation lines, it's single-line.
    if value_lines.is_empty() {
        return ListStyle::SingleLine;
    }

    // Multi-line: examine continuation lines for comma style.
    let mut has_leading_comma = false;
    let mut has_trailing_comma = false;
    let mut has_any_comma = false;

    for &vl_id in &value_lines {
        let vl = cst.node(vl_id);
        let text = vl.content_span.slice(&cst.source);
        let trimmed = text.trim();

        if trimmed.starts_with(',') {
            has_leading_comma = true;
            has_any_comma = true;
        }
        if trimmed.ends_with(',') {
            has_trailing_comma = true;
            has_any_comma = true;
        }
    }

    // Also check the inline value for trailing comma (Style C can have first
    // item on the field line itself ending with comma).
    if let Some(fv) = node.field_value {
        let fv_text = fv.slice(&cst.source).trim();
        if fv_text.ends_with(',') {
            has_trailing_comma = true;
            has_any_comma = true;
        }
    }

    if !has_any_comma {
        return ListStyle::NoComma;
    }
    if has_leading_comma {
        return ListStyle::LeadingComma;
    }
    if has_trailing_comma {
        return ListStyle::TrailingComma;
    }

    // Fallback: if commas are present but we couldn't classify, default to
    // trailing comma (most common multi-line style after leading).
    ListStyle::TrailingComma
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the "item name" from a list item string. For dependencies, this
/// is the package name (everything before version constraints). For modules,
/// it's the module name itself.
fn item_name(item: &str) -> &str {
    let trimmed = item.trim().trim_start_matches(',').trim();
    // The item name is the first word (letters, digits, hyphens, underscores, dots).
    let end = trimmed
        .find(|c: char| c.is_whitespace() || c == '>' || c == '<' || c == '=' || c == '^')
        .unwrap_or(trimmed.len());
    let name = &trimmed[..end];
    name.trim_end_matches(',')
}

/// Extract a clean item text for comparison, stripping leading/trailing commas
/// and whitespace.
fn clean_item_text(text: &str) -> &str {
    text.trim()
        .trim_start_matches(',')
        .trim_end_matches(',')
        .trim()
}

/// Gather all items from a field as (item_text, source_range) pairs.
/// The source_range covers the full line/segment including indentation and
/// newlines.
fn gather_items(cst: &CabalCst, field_node: NodeId) -> Vec<(String, Span)> {
    let node = cst.node(field_node);
    let mut items = Vec::new();

    // Inline value items (for single-line style).
    if let Some(fv) = node.field_value {
        let fv_text = fv.slice(&cst.source);
        if !fv_text.trim().is_empty() {
            items.push((fv_text.to_owned(), fv));
        }
    }

    // ValueLine children.
    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            let text = child.content_span.slice(&cst.source).to_owned();
            items.push((text, child.span));
        }
    }

    items
}

/// Find the indentation string used by existing value lines. Returns the
/// whitespace prefix of the first value line, or a default based on the
/// field's own indent.
fn detect_item_indent(cst: &CabalCst, field_node: NodeId) -> String {
    let node = cst.node(field_node);

    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            // The span includes leading trivia (whitespace). The content_span
            // starts at the actual content. The difference is the indentation.
            let line_start = child.span.start;
            let content_start = child.content_span.start;
            if content_start > line_start {
                return cst.source[line_start..content_start].to_owned();
            }
            // If span == content_span, use the indent level.
            return " ".repeat(child.indent);
        }
    }

    // No existing children: use field indent + 2 spaces.
    let field_indent = node.indent;
    " ".repeat(field_indent + 2)
}

/// Find the byte offset where the field's value area ends (after the last
/// ValueLine child, or after the inline value if no children). This is the
/// insertion point for appending new items.
fn field_value_end(cst: &CabalCst, field_node: NodeId) -> usize {
    let node = cst.node(field_node);

    // If there are ValueLine children, the end is after the last one.
    let value_lines: Vec<NodeId> = node
        .children
        .iter()
        .copied()
        .filter(|&id| cst.node(id).kind == CstNodeKind::ValueLine)
        .collect();

    if let Some(&last_vl) = value_lines.last() {
        return cst.node(last_vl).span.end;
    }

    // No children: end is after the trailing trivia of the field node itself.
    node.span.end
}

/// Find the insertion point and text for adding an item at a specific index
/// in the sorted list of items.
fn find_sorted_insert_index(items: &[(String, Span)], new_item: &str) -> usize {
    let new_name = item_name(new_item).to_lowercase();
    for (i, (text, _)) in items.iter().enumerate() {
        let existing_name = item_name(clean_item_text(text)).to_lowercase();
        if new_name < existing_name {
            return i;
        }
    }
    items.len()
}

// ---------------------------------------------------------------------------
// High-level edit operations
// ---------------------------------------------------------------------------

/// Add an item to a list field (e.g., add a dependency to build-depends).
///
/// Respects the detected list style. If `sort` is true, inserts alphabetically
/// by item name; otherwise appends at the end.
pub fn add_list_item(cst: &CabalCst, field_node: NodeId, item: &str, sort: bool) -> Vec<TextEdit> {
    let style = detect_list_style(cst, field_node);
    let items = gather_items(cst, field_node);

    // Empty field (no items at all).
    if items.is_empty() {
        return add_item_to_empty_field(cst, field_node, item, style);
    }

    let insert_idx = if sort {
        find_sorted_insert_index(&items, item)
    } else {
        items.len()
    };

    match style {
        ListStyle::SingleLine => add_item_single_line(cst, field_node, &items, item, insert_idx),
        ListStyle::LeadingComma => {
            add_item_leading_comma(cst, field_node, &items, item, insert_idx)
        }
        ListStyle::TrailingComma => {
            add_item_trailing_comma(cst, field_node, &items, item, insert_idx)
        }
        ListStyle::NoComma => add_item_no_comma(cst, field_node, &items, item, insert_idx),
    }
}

/// Add an item to a field that currently has no items.
fn add_item_to_empty_field(
    cst: &CabalCst,
    field_node: NodeId,
    item: &str,
    style: ListStyle,
) -> Vec<TextEdit> {
    let node = cst.node(field_node);

    match style {
        ListStyle::SingleLine => {
            // The field line is `field-name:\n` — insert value after the colon.
            // Find the colon position from the content_span.
            let content_end = node.content_span.end;
            vec![TextEdit {
                range: Span::new(content_end, content_end),
                replacement: format!(" {item}"),
            }]
        }
        _ => {
            // Multi-line: add after the field line with proper indentation.
            let indent = detect_item_indent(cst, field_node);
            let end = field_value_end(cst, field_node);
            vec![TextEdit {
                range: Span::new(end, end),
                replacement: format!("{indent}{item}\n"),
            }]
        }
    }
}

/// Add item to a single-line comma-separated field.
fn add_item_single_line(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    item: &str,
    insert_idx: usize,
) -> Vec<TextEdit> {
    let node = cst.node(field_node);

    // The entire value is in the field_value span.
    if let Some(fv) = node.field_value {
        let fv_text = fv.slice(&cst.source);

        // Parse individual items from the single-line value.
        let parts: Vec<&str> = fv_text.split(',').collect();

        if insert_idx >= parts.len() || insert_idx >= items.len() {
            // Append at the end.
            vec![TextEdit {
                range: Span::new(fv.end, fv.end),
                replacement: format!(", {item}"),
            }]
        } else {
            // Find the byte offset of the insert position within fv_text.
            // We need to find where the nth comma-separated item starts.
            let mut offset = 0;
            for (i, part) in parts.iter().enumerate() {
                if i == insert_idx {
                    break;
                }
                offset += part.len() + 1; // +1 for the comma
            }
            let insert_offset = fv.start + offset;
            vec![TextEdit {
                range: Span::new(insert_offset, insert_offset),
                replacement: format!("{item}, "),
            }]
        }
    } else {
        // No field value — shouldn't happen since items is non-empty, but
        // handle gracefully.
        add_item_to_empty_field(cst, field_node, item, ListStyle::SingleLine)
    }
}

/// Add item to a leading-comma multi-line field.
///
/// Leading comma style:
/// ```text
/// build-depends:
///     base >=4.14
///   , text >=2.0
///   , aeson ^>=2.2
/// ```
/// The first item has no leading comma; subsequent items start with `, `.
fn add_item_leading_comma(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    item: &str,
    insert_idx: usize,
) -> Vec<TextEdit> {
    let indent = detect_item_indent(cst, field_node);

    // For leading-comma, we need to figure out the comma+indent pattern.
    // Typically:
    //   - First item:  "    base >=4.14"  (deeper indent, no comma)
    //   - Other items: "  , text >=2.0"   (comma indent, then ", item")
    //
    // Detect the comma indent from an existing non-first ValueLine.
    let node = cst.node(field_node);
    let value_lines: Vec<NodeId> = node
        .children
        .iter()
        .copied()
        .filter(|&id| cst.node(id).kind == CstNodeKind::ValueLine)
        .collect();

    // Determine if the first item is the inline value or the first ValueLine.
    let has_inline = node.field_value.is_some()
        && !node
            .field_value
            .unwrap()
            .slice(&cst.source)
            .trim()
            .is_empty();

    // Find the comma prefix used by non-first items.
    let comma_prefix = find_leading_comma_prefix(cst, &value_lines, &indent);

    if insert_idx == 0 {
        // Inserting at the very beginning.
        if has_inline {
            // Replace the inline value — insert before it with the new item
            // and demote the old first item to have a leading comma.
            let fv = node.field_value.unwrap();
            let fv_text = fv.slice(&cst.source).to_owned();
            let old_first_clean = clean_item_text(&fv_text);

            // The first item uses deeper indent (from the existing inline value position).
            // We need to move the inline value to a ValueLine and insert our new one.
            // The simplest approach: replace inline value with new item, and insert
            // old first as leading-comma line.
            let first_vl_end = if value_lines.is_empty() {
                field_value_end(cst, field_node)
            } else {
                // Insert right before the first ValueLine.
                cst.node(value_lines[0]).span.start
            };

            vec![
                TextEdit {
                    range: fv,
                    replacement: item.to_owned(),
                },
                TextEdit {
                    range: Span::new(first_vl_end, first_vl_end),
                    replacement: format!("{comma_prefix}{old_first_clean}\n"),
                },
            ]
        } else if !value_lines.is_empty() {
            // First item is the first ValueLine. We need to:
            // 1. Replace the first ValueLine content with our new item (using
            //    the deeper first-item indent).
            // 2. Insert the old first item as a leading-comma line after.
            let first_vl = cst.node(value_lines[0]);
            let first_text = first_vl.content_span.slice(&cst.source);
            let old_first_clean = clean_item_text(first_text).to_owned();

            // The first item's full indent (no comma).
            let first_item_indent = &cst.source[first_vl.span.start..first_vl.content_span.start];

            vec![TextEdit {
                range: first_vl.span,
                replacement: format!(
                    "{first_item_indent}{item}\n{comma_prefix}{old_first_clean}\n"
                ),
            }]
        } else {
            // No items at all — shouldn't reach here since items is non-empty.
            add_item_to_empty_field(cst, field_node, item, ListStyle::LeadingComma)
        }
    } else if insert_idx >= items.len() {
        // Append at end.
        let end = field_value_end(cst, field_node);
        vec![TextEdit {
            range: Span::new(end, end),
            replacement: format!("{comma_prefix}{item}\n"),
        }]
    } else {
        // Insert in the middle. The new item gets a leading comma and goes
        // before the item at insert_idx.
        let target_item = &items[insert_idx];
        let target_span = target_item.1;
        vec![TextEdit {
            range: Span::new(target_span.start, target_span.start),
            replacement: format!("{comma_prefix}{item}\n"),
        }]
    }
}

/// Find the leading-comma prefix string (e.g., "  , ") from existing value lines.
fn find_leading_comma_prefix(
    cst: &CabalCst,
    value_lines: &[NodeId],
    default_indent: &str,
) -> String {
    for &vl_id in value_lines {
        let vl = cst.node(vl_id);
        let text = vl.content_span.slice(&cst.source);
        let trimmed = text.trim();
        if trimmed.starts_with(',') {
            // This line has a leading comma. The full prefix is the indentation
            // + the comma + space.
            // content_span starts at the comma. Find where the actual item text
            // starts (after ", ").
            let after_comma = trimmed.trim_start_matches(',').len();
            let comma_and_space_len = trimmed.len() - after_comma;
            let prefix_end = vl.content_span.start + comma_and_space_len;
            return cst.source[vl.span.start..prefix_end].to_owned();
        }
    }
    // Fallback: use the default indent with ", " prefix.
    format!("{default_indent}, ")
}

/// Add item to a trailing-comma multi-line field.
///
/// Trailing comma style:
/// ```text
/// build-depends:
///     base >=4.14,
///     text >=2.0,
///     aeson ^>=2.2
/// ```
fn add_item_trailing_comma(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    item: &str,
    insert_idx: usize,
) -> Vec<TextEdit> {
    let indent = detect_item_indent(cst, field_node);

    if insert_idx >= items.len() {
        // Append at end in trailing-comma style.
        let last = &items[items.len() - 1];
        let last_span = last.1;
        let last_node_text = last.0.trim();
        let last_has_comma = last_node_text.ends_with(',');

        let mut edits = Vec::new();

        // Add trailing comma to the current last item if it doesn't have one.
        if !last_has_comma {
            let last_content_end = find_content_end_in_span(cst, last_span);
            edits.push(TextEdit {
                range: Span::new(last_content_end, last_content_end),
                replacement: ",".to_owned(),
            });
        }

        // Add the new item. If the existing last item already had a trailing
        // comma (meaning this style uses trailing commas on every line),
        // add our new item with a trailing comma too for consistency.
        let new_item = if last_has_comma {
            format!("{indent}{item},\n")
        } else {
            format!("{indent}{item}\n")
        };

        let end = field_value_end(cst, field_node);
        edits.push(TextEdit {
            range: Span::new(end, end),
            replacement: new_item,
        });

        edits
    } else if insert_idx == 0 {
        // Insert at the beginning.
        let first = &items[0];
        let first_span = first.1;
        vec![TextEdit {
            range: Span::new(first_span.start, first_span.start),
            replacement: format!("{indent}{item},\n"),
        }]
    } else {
        // Insert in the middle. The item at insert_idx-1 should already have
        // a trailing comma.
        let target = &items[insert_idx];
        let target_span = target.1;
        vec![TextEdit {
            range: Span::new(target_span.start, target_span.start),
            replacement: format!("{indent}{item},\n"),
        }]
    }
}

/// Add item to a no-comma multi-line field (e.g., exposed-modules).
fn add_item_no_comma(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    item: &str,
    insert_idx: usize,
) -> Vec<TextEdit> {
    let indent = detect_item_indent(cst, field_node);

    if insert_idx >= items.len() {
        // Append at end.
        let end = field_value_end(cst, field_node);
        vec![TextEdit {
            range: Span::new(end, end),
            replacement: format!("{indent}{item}\n"),
        }]
    } else {
        // Insert before the item at insert_idx.
        let target = &items[insert_idx];
        let target_span = target.1;
        vec![TextEdit {
            range: Span::new(target_span.start, target_span.start),
            replacement: format!("{indent}{item}\n"),
        }]
    }
}

/// Find the end of the actual text content within a span (before trailing
/// whitespace/newline).
fn find_content_end_in_span(cst: &CabalCst, span: Span) -> usize {
    let text = &cst.source[span.start..span.end];
    let trimmed_len = text.trim_end().len();
    span.start + trimmed_len
}

// ---------------------------------------------------------------------------
// Remove list item
// ---------------------------------------------------------------------------

/// Remove an item from a list field by prefix match on the item name.
///
/// For dependencies, `item_prefix` is typically the package name. The
/// removal matches if the item's name starts with `item_prefix`.
pub fn remove_list_item(cst: &CabalCst, field_node: NodeId, item_prefix: &str) -> Vec<TextEdit> {
    let style = detect_list_style(cst, field_node);

    // Single-line fields need special handling: the whole value is one span,
    // so we search within the comma-separated text directly.
    if style == ListStyle::SingleLine {
        return remove_item_single_line(cst, field_node, item_prefix);
    }

    let items = gather_items(cst, field_node);

    let prefix_lower = item_prefix.to_lowercase();

    // Find the index of the item to remove.
    let remove_idx = items.iter().position(|(text, _)| {
        let name = item_name(clean_item_text(text)).to_lowercase();
        name == prefix_lower || name.starts_with(&prefix_lower)
    });

    let remove_idx = match remove_idx {
        Some(idx) => idx,
        None => return Vec::new(), // Item not found.
    };

    match style {
        ListStyle::SingleLine => unreachable!(),
        ListStyle::LeadingComma => remove_item_leading_comma(cst, field_node, &items, remove_idx),
        ListStyle::TrailingComma => remove_item_trailing_comma(cst, field_node, &items, remove_idx),
        ListStyle::NoComma => remove_item_no_comma(&items, remove_idx),
    }
}

/// Remove an item from a single-line comma-separated field.
///
/// Searches within the comma-separated value text using `item_prefix`.
fn remove_item_single_line(cst: &CabalCst, field_node: NodeId, item_prefix: &str) -> Vec<TextEdit> {
    let node = cst.node(field_node);
    if let Some(fv) = node.field_value {
        let fv_text = fv.slice(&cst.source);
        let parts: Vec<&str> = fv_text.split(',').collect();

        let prefix_lower = item_prefix.to_lowercase();

        // Find which comma-separated part matches.
        let part_idx = parts.iter().position(|part| {
            let name = item_name(part.trim()).to_lowercase();
            name == prefix_lower || name.starts_with(&prefix_lower)
        });

        let part_idx = match part_idx {
            Some(idx) => idx,
            None => return Vec::new(),
        };

        if parts.len() <= 1 {
            // Only one item — remove the entire value.
            let name_end = node
                .field_name
                .map(|s| s.end)
                .unwrap_or(node.content_span.start);
            let colon_end = {
                let after_name = &cst.source[name_end..node.content_span.end];
                let colon_pos = after_name.find(':').map(|p| name_end + p + 1);
                colon_pos.unwrap_or(node.content_span.end)
            };
            return vec![TextEdit {
                range: Span::new(colon_end, fv.end),
                replacement: String::new(),
            }];
        }

        // Multiple items: rebuild the value string without the removed item.
        let mut new_parts: Vec<&str> = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if i != part_idx {
                new_parts.push(part.trim());
            }
        }
        let new_value = new_parts.join(", ");

        vec![TextEdit {
            range: fv,
            replacement: new_value,
        }]
    } else {
        Vec::new()
    }
}

/// Remove an item from a leading-comma multi-line field.
fn remove_item_leading_comma(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    remove_idx: usize,
) -> Vec<TextEdit> {
    let node = cst.node(field_node);
    let has_inline = node.field_value.is_some()
        && !node
            .field_value
            .unwrap()
            .slice(&cst.source)
            .trim()
            .is_empty();

    if items.len() == 1 {
        // Removing the only item.
        let (_, span) = &items[0];
        if has_inline {
            // Clear the inline value.
            let fv = node.field_value.unwrap();
            return vec![TextEdit {
                range: fv,
                replacement: String::new(),
            }];
        }
        // Remove the ValueLine.
        return vec![TextEdit {
            range: *span,
            replacement: String::new(),
        }];
    }

    if remove_idx == 0 {
        // Removing the first item (which has no leading comma).
        let first_span = items[0].1;

        if has_inline {
            // The first item is inline. We need to replace it with the second
            // item (which currently has a leading comma) and remove the second
            // item's line.
            let second_text = clean_item_text(&items[1].0);
            let second_span = items[1].1;
            let fv = node.field_value.unwrap();

            return vec![
                TextEdit {
                    range: fv,
                    replacement: second_text.to_owned(),
                },
                TextEdit {
                    range: second_span,
                    replacement: String::new(),
                },
            ];
        }

        // First item is a ValueLine. Remove it and strip the leading comma
        // from the second item (which becomes the new first).
        let second_vl_id = node
            .children
            .iter()
            .copied()
            .filter(|&id| cst.node(id).kind == CstNodeKind::ValueLine)
            .nth(1)
            .unwrap_or(node.children[1]);

        let second_vl = cst.node(second_vl_id);
        let second_text = second_vl.content_span.slice(&cst.source);
        let clean = clean_item_text(second_text);

        // Determine the first item's indent (deeper, no comma).
        let first_vl_id = node
            .children
            .iter()
            .copied()
            .find(|&id| cst.node(id).kind == CstNodeKind::ValueLine)
            .unwrap();
        let first_vl = cst.node(first_vl_id);
        let first_indent = &cst.source[first_vl.span.start..first_vl.content_span.start];

        vec![
            TextEdit {
                range: first_span,
                replacement: String::new(),
            },
            TextEdit {
                range: items[1].1,
                replacement: format!("{first_indent}{clean}\n"),
            },
        ]
    } else {
        // Removing a non-first item (which has a leading comma). Just remove
        // the entire line.
        let span = items[remove_idx].1;
        vec![TextEdit {
            range: span,
            replacement: String::new(),
        }]
    }
}

/// Remove an item from a trailing-comma multi-line field.
fn remove_item_trailing_comma(
    cst: &CabalCst,
    field_node: NodeId,
    items: &[(String, Span)],
    remove_idx: usize,
) -> Vec<TextEdit> {
    let _ = cst;
    let _ = field_node;

    if items.len() == 1 {
        // Removing the only item.
        let span = items[0].1;
        return vec![TextEdit {
            range: span,
            replacement: String::new(),
        }];
    }

    if remove_idx == items.len() - 1 {
        // Removing the last item in trailing-comma style.
        let last_span = items[remove_idx].1;
        let last_text = &items[remove_idx].0;
        let last_has_comma = last_text.trim().ends_with(',');

        if !last_has_comma && items.len() > 1 {
            // The item we're removing has no trailing comma (it was added
            // as the last item). The previous item had a comma added by
            // the add operation to maintain trailing-comma style. We need
            // to remove that comma to restore the original state.
            let prev_text = &items[remove_idx - 1].0;
            let prev_span = items[remove_idx - 1].1;

            if prev_text.trim().ends_with(',') {
                let content_end = find_content_end_in_span(cst, prev_span);
                return vec![
                    TextEdit {
                        range: Span::new(content_end - 1, content_end),
                        replacement: String::new(),
                    },
                    TextEdit {
                        range: last_span,
                        replacement: String::new(),
                    },
                ];
            }
        }

        // Either the last item has a trailing comma (original style had
        // trailing commas on every item), or there's no previous item.
        // Just remove the line.
        return vec![TextEdit {
            range: last_span,
            replacement: String::new(),
        }];
    }

    // Removing a non-last item. Just remove the entire line (it has a trailing
    // comma, and the next item also has one or is the last without one — either
    // way, removal is clean).
    let span = items[remove_idx].1;
    vec![TextEdit {
        range: span,
        replacement: String::new(),
    }]
}

/// Remove an item from a no-comma multi-line field.
fn remove_item_no_comma(items: &[(String, Span)], remove_idx: usize) -> Vec<TextEdit> {
    let span = items[remove_idx].1;
    vec![TextEdit {
        range: span,
        replacement: String::new(),
    }]
}

// ---------------------------------------------------------------------------
// Scalar field editing
// ---------------------------------------------------------------------------

/// Set a simple scalar field value. Replaces the current value text.
///
/// If the field has no value (e.g., `field-name:\n`), inserts the value after
/// the colon with a space.
pub fn set_field_value(cst: &CabalCst, field_node: NodeId, value: &str) -> TextEdit {
    let node = cst.node(field_node);
    debug_assert_eq!(node.kind, CstNodeKind::Field);

    if let Some(fv) = node.field_value {
        // Replace existing value.
        TextEdit {
            range: fv,
            replacement: value.to_owned(),
        }
    } else {
        // No existing value — insert after the content_span (which ends at the
        // colon).
        let insert_at = node.content_span.end;
        TextEdit {
            range: Span::new(insert_at, insert_at),
            replacement: format!(" {value}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Root-level (top-level metadata) editing
// ---------------------------------------------------------------------------

/// Add a new top-level metadata field to the root of the file.
///
/// The field is inserted after the last existing top-level field and before
/// any section (library, executable, etc.). Formatted as
/// `{field_name}: {field_value}\n` with no indentation (top-level).
pub fn add_field_to_root(cst: &CabalCst, field_name: &str, field_value: &str) -> TextEdit {
    let root = cst.node(cst.root);

    // Find the insertion point: after the last top-level Field node and before
    // the first Section node.
    let mut insert_at = 0usize;
    for &child_id in &root.children {
        let child = cst.node(child_id);
        match child.kind {
            CstNodeKind::Field | CstNodeKind::Comment | CstNodeKind::BlankLine => {
                insert_at = child.span.end;
            }
            CstNodeKind::Section => {
                // Insert before the first section.
                break;
            }
            _ => {
                insert_at = child.span.end;
            }
        }
    }

    TextEdit {
        range: Span::new(insert_at, insert_at),
        replacement: format!("{field_name}: {field_value}\n"),
    }
}

// ---------------------------------------------------------------------------
// Section-level editing
// ---------------------------------------------------------------------------

/// Add a new field to a section at the end (before any conditionals).
///
/// The field is formatted as `{indent}{field_name}: {field_value}\n` where
/// `indent` matches the existing fields in the section.
pub fn add_field_to_section(
    cst: &CabalCst,
    section_node: NodeId,
    field_name: &str,
    field_value: &str,
) -> TextEdit {
    let section = cst.node(section_node);
    debug_assert_eq!(section.kind, CstNodeKind::Section);

    // Determine the indentation used by existing fields in this section.
    let field_indent = find_section_field_indent(cst, section_node);

    // Find the insertion point: after the last non-conditional child.
    let insert_at = find_field_insertion_point(cst, section_node);

    TextEdit {
        range: Span::new(insert_at, insert_at),
        replacement: format!("{field_indent}{field_name}: {field_value}\n"),
    }
}

/// Find the indentation string used by fields in a section.
fn find_section_field_indent(cst: &CabalCst, section_node: NodeId) -> String {
    let section = cst.node(section_node);
    for &child_id in &section.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field || child.kind == CstNodeKind::Import {
            return " ".repeat(child.indent);
        }
    }
    // Default: section indent + 2.
    " ".repeat(section.indent + 2)
}

/// Find the insertion point for a new field in a section (after the last
/// regular field, before conditionals).
fn find_field_insertion_point(cst: &CabalCst, section_node: NodeId) -> usize {
    let section = cst.node(section_node);
    let mut last_field_end = section.span.start;

    // Find trailing trivia end of the section header if no children.
    if section.children.is_empty() {
        return section.span.end;
    }

    for &child_id in &section.children {
        let child = cst.node(child_id);
        match child.kind {
            CstNodeKind::Field
            | CstNodeKind::Import
            | CstNodeKind::Comment
            | CstNodeKind::BlankLine => {
                last_field_end = child.span.end;
            }
            CstNodeKind::Conditional => {
                // Insert before the first conditional.
                break;
            }
            _ => {
                last_field_end = child.span.end;
            }
        }
    }

    last_field_end
}

/// Add a new top-level section to the end of the file.
///
/// `keyword` is the section type (e.g., `"library"`, `"executable"`).
/// `name` is the optional section argument (e.g., `"my-exe"`).
/// `fields` is a list of `(field_name, field_value)` pairs.
/// `indent` is the number of spaces to indent fields within the section.
pub fn add_section(
    cst: &CabalCst,
    keyword: &str,
    name: Option<&str>,
    fields: &[(&str, &str)],
    indent: usize,
) -> TextEdit {
    let insert_at = cst.source.len();
    let indent_str = " ".repeat(indent);

    let mut text = String::new();

    // Ensure there's a blank line before the new section if the file doesn't
    // end with one.
    if !cst.source.is_empty() && !cst.source.ends_with('\n') {
        text.push('\n');
    }
    if !cst.source.is_empty() && !cst.source.ends_with("\n\n") {
        text.push('\n');
    }

    // Section header.
    text.push_str(keyword);
    if let Some(n) = name {
        text.push(' ');
        text.push_str(n);
    }
    text.push('\n');

    // Fields.
    for (fname, fvalue) in fields {
        text.push_str(&indent_str);
        text.push_str(fname);
        text.push_str(": ");
        text.push_str(fvalue);
        text.push('\n');
    }

    TextEdit {
        range: Span::new(insert_at, insert_at),
        replacement: text,
    }
}

// ---------------------------------------------------------------------------
// Convenience: find a field node by name within a section
// ---------------------------------------------------------------------------

/// Find a field node within a section (or the root) by field name.
///
/// The search is case-insensitive and treats hyphens and underscores as
/// equivalent (matching `.cabal` conventions).
pub fn find_field(cst: &CabalCst, parent_node: NodeId, field_name: &str) -> Option<NodeId> {
    let parent = cst.node(parent_node);
    let normalized = normalize_field_name(field_name);

    for &child_id in &parent.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field {
            if let Some(name_span) = child.field_name {
                let name = name_span.slice(&cst.source);
                if normalize_field_name(name) == normalized {
                    return Some(child_id);
                }
            }
        }
    }
    None
}

/// Find a section node by keyword and optional name.
pub fn find_section(cst: &CabalCst, keyword: &str, name: Option<&str>) -> Option<NodeId> {
    let root = cst.node(cst.root);
    for &child_id in &root.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Section {
            if let Some(kw_span) = child.section_keyword {
                let kw = kw_span.slice(&cst.source);
                if kw.eq_ignore_ascii_case(keyword) {
                    match name {
                        None => return Some(child_id),
                        Some(n) => {
                            if let Some(arg_span) = child.section_arg {
                                if arg_span.slice(&cst.source).eq_ignore_ascii_case(n) {
                                    return Some(child_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Normalize a field name: lowercase, replace underscores with hyphens.
fn normalize_field_name(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// Helper: parse source, apply edits, return new source.
    fn apply_edits(source: &str, edits: Vec<TextEdit>) -> String {
        let mut batch = EditBatch::new();
        batch.add_all(edits);
        batch.apply(source)
    }

    // -- EditBatch tests ---------------------------------------------------

    #[test]
    fn edit_batch_empty() {
        let source = "hello world";
        let batch = EditBatch::new();
        assert_eq!(batch.apply(source), "hello world");
    }

    #[test]
    fn edit_batch_single_insert() {
        let source = "hello world";
        let mut batch = EditBatch::new();
        batch.add(TextEdit {
            range: Span::new(5, 5),
            replacement: ",".to_owned(),
        });
        assert_eq!(batch.apply(source), "hello, world");
    }

    #[test]
    fn edit_batch_single_replace() {
        let source = "hello world";
        let mut batch = EditBatch::new();
        batch.add(TextEdit {
            range: Span::new(6, 11),
            replacement: "rust".to_owned(),
        });
        assert_eq!(batch.apply(source), "hello rust");
    }

    #[test]
    fn edit_batch_single_delete() {
        let source = "hello world";
        let mut batch = EditBatch::new();
        batch.add(TextEdit {
            range: Span::new(5, 6),
            replacement: String::new(),
        });
        assert_eq!(batch.apply(source), "helloworld");
    }

    #[test]
    fn edit_batch_multiple_non_overlapping() {
        let source = "aaa bbb ccc";
        let mut batch = EditBatch::new();
        batch.add(TextEdit {
            range: Span::new(0, 3),
            replacement: "xxx".to_owned(),
        });
        batch.add(TextEdit {
            range: Span::new(8, 11),
            replacement: "zzz".to_owned(),
        });
        assert_eq!(batch.apply(source), "xxx bbb zzz");
    }

    #[test]
    #[should_panic(expected = "overlapping edits")]
    fn edit_batch_overlapping_panics() {
        let source = "hello world";
        let mut batch = EditBatch::new();
        batch.add(TextEdit {
            range: Span::new(0, 7),
            replacement: "hi".to_owned(),
        });
        batch.add(TextEdit {
            range: Span::new(5, 11),
            replacement: "there".to_owned(),
        });
        batch.apply(source);
    }

    // -- List style detection tests ----------------------------------------

    #[test]
    fn detect_style_single_line() {
        let src = "\
library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        assert_eq!(detect_list_style(&result.cst, field), ListStyle::SingleLine);
    }

    #[test]
    fn detect_style_leading_comma() {
        let src = "\
library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        assert_eq!(
            detect_list_style(&result.cst, field),
            ListStyle::LeadingComma
        );
    }

    #[test]
    fn detect_style_trailing_comma() {
        let src = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        assert_eq!(
            detect_list_style(&result.cst, field),
            ListStyle::TrailingComma
        );
    }

    #[test]
    fn detect_style_no_comma() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();
        assert_eq!(detect_list_style(&result.cst, field), ListStyle::NoComma);
    }

    // -- set_field_value tests ---------------------------------------------

    #[test]
    fn set_scalar_field_value() {
        let src = "name: foo\nversion: 0.1.0.0\n";
        let result = parse::parse(src);
        let field = find_field(&result.cst, result.cst.root, "version").unwrap();
        let edit = set_field_value(&result.cst, field, "1.0.0.0");
        let new_src = apply_edits(src, vec![edit]);
        assert_eq!(new_src, "name: foo\nversion: 1.0.0.0\n");
    }

    #[test]
    fn set_field_value_empty_field() {
        let src = "name:\nversion: 0.1.0.0\n";
        let result = parse::parse(src);
        let field = find_field(&result.cst, result.cst.root, "name").unwrap();
        let edit = set_field_value(&result.cst, field, "my-package");
        let new_src = apply_edits(src, vec![edit]);
        assert_eq!(new_src, "name: my-package\nversion: 0.1.0.0\n");
    }

    // -- add_list_item tests (NoComma style) -------------------------------

    #[test]
    fn add_module_no_comma() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();
        let edits = add_list_item(&result.cst, field, "Data.List", true);
        let new_src = apply_edits(src, edits);

        // Data.List should be inserted between Data.Map and Data.Set (sorted).
        assert!(new_src.contains("Data.List"));
        // Verify the result parses cleanly.
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    #[test]
    fn add_module_no_comma_end() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();
        let edits = add_list_item(&result.cst, field, "Data.Text", true);
        let new_src = apply_edits(src, edits);

        // Data.Text should appear after Data.Set (sorted).
        let map_pos = new_src.find("Data.Map").unwrap();
        let set_pos = new_src.find("Data.Set").unwrap();
        let text_pos = new_src.find("Data.Text").unwrap();
        assert!(map_pos < set_pos);
        assert!(set_pos < text_pos);
    }

    // -- add_list_item tests (trailing comma style) ------------------------

    #[test]
    fn add_dep_trailing_comma_end() {
        let src = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = add_list_item(&result.cst, field, "zlib ^>=0.7", true);
        let new_src = apply_edits(src, edits);

        assert!(new_src.contains("zlib ^>=0.7"));
        // aeson should now have a trailing comma.
        assert!(new_src.contains("aeson ^>=2.2,"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    // -- add_list_item tests (leading comma style) -------------------------

    #[test]
    fn add_dep_leading_comma_end() {
        let src = "\
library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = add_list_item(&result.cst, field, "zlib ^>=0.7", true);
        let new_src = apply_edits(src, edits);

        assert!(new_src.contains("zlib ^>=0.7"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    // -- add_list_item tests (single line style) ---------------------------

    #[test]
    fn add_dep_single_line_end() {
        let src = "\
library
  build-depends: base >=4.14, text >=2.0
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = add_list_item(&result.cst, field, "aeson ^>=2.2", false);
        let new_src = apply_edits(src, edits);

        assert!(new_src.contains("aeson ^>=2.2"));
        assert!(new_src.contains("text >=2.0, aeson ^>=2.2"));
    }

    // -- remove_list_item tests --------------------------------------------

    #[test]
    fn remove_module_no_comma() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
    Data.Text
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();
        let edits = remove_list_item(&result.cst, field, "Data.Set");
        let new_src = apply_edits(src, edits);

        assert!(!new_src.contains("Data.Set"));
        assert!(new_src.contains("Data.Map"));
        assert!(new_src.contains("Data.Text"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    #[test]
    fn remove_dep_trailing_comma_middle() {
        let src = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = remove_list_item(&result.cst, field, "text");
        let new_src = apply_edits(src, edits);

        assert!(!new_src.contains("text"));
        assert!(new_src.contains("base"));
        assert!(new_src.contains("aeson"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    #[test]
    fn remove_dep_trailing_comma_last() {
        let src = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = remove_list_item(&result.cst, field, "aeson");
        let new_src = apply_edits(src, edits);

        assert!(!new_src.contains("aeson"));
        assert!(new_src.contains("base"));
        assert!(new_src.contains("text"));
        // text should no longer have a trailing comma (it's now the last item).
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    #[test]
    fn remove_dep_single_line_middle() {
        let src = "\
library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = remove_list_item(&result.cst, field, "text");
        let new_src = apply_edits(src, edits);

        assert!(!new_src.contains("text"));
        assert!(new_src.contains("base >=4.14, aeson ^>=2.2"));
    }

    // -- add_field_to_section tests ----------------------------------------

    #[test]
    fn add_field_to_section_basic() {
        let src = "\
library
  exposed-modules: Foo
  build-depends: base
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let edit = add_field_to_section(&result.cst, section, "default-language", "GHC2021");
        let new_src = apply_edits(src, vec![edit]);

        assert!(new_src.contains("default-language: GHC2021"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    // -- add_section tests -------------------------------------------------

    #[test]
    fn add_new_section() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0
";
        let result = parse::parse(src);
        let edit = add_section(
            &result.cst,
            "library",
            None,
            &[
                ("exposed-modules", "Foo"),
                ("build-depends", "base"),
                ("hs-source-dirs", "src"),
            ],
            2,
        );
        let new_src = apply_edits(src, vec![edit]);

        assert!(new_src.contains("library\n"));
        assert!(new_src.contains("  exposed-modules: Foo\n"));
        assert!(new_src.contains("  build-depends: base\n"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    #[test]
    fn add_named_section() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0
";
        let result = parse::parse(src);
        let edit = add_section(
            &result.cst,
            "executable",
            Some("my-exe"),
            &[("main-is", "Main.hs"), ("build-depends", "base, foo")],
            2,
        );
        let new_src = apply_edits(src, vec![edit]);

        assert!(new_src.contains("executable my-exe\n"));
        assert!(new_src.contains("  main-is: Main.hs\n"));
    }

    // -- find_field / find_section tests -----------------------------------

    #[test]
    fn find_field_case_insensitive() {
        let src = "Name: foo\nVersion: 0.1.0.0\n";
        let result = parse::parse(src);
        assert!(find_field(&result.cst, result.cst.root, "name").is_some());
        assert!(find_field(&result.cst, result.cst.root, "NAME").is_some());
    }

    #[test]
    fn find_field_underscore_hyphen() {
        let src = "build-depends: base\n";
        let result = parse::parse(src);
        assert!(find_field(&result.cst, result.cst.root, "build_depends").is_some());
        assert!(find_field(&result.cst, result.cst.root, "build-depends").is_some());
    }

    #[test]
    fn find_section_library() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
";
        let result = parse::parse(src);
        assert!(find_section(&result.cst, "library", None).is_some());
    }

    #[test]
    fn find_section_named_executable() {
        let src = "\
executable my-exe
  main-is: Main.hs
";
        let result = parse::parse(src);
        assert!(find_section(&result.cst, "executable", Some("my-exe")).is_some());
        assert!(find_section(&result.cst, "executable", Some("other")).is_none());
    }

    // -- Round-trip edit tests (add then remove) ---------------------------

    #[test]
    fn round_trip_add_remove_no_comma() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();

        // Add.
        let edits = add_list_item(&result.cst, field, "Data.List", true);
        let added_src = apply_edits(src, edits);
        assert!(added_src.contains("Data.List"));

        // Remove.
        let result2 = parse::parse(&added_src);
        let section2 = result2.cst.node(result2.cst.root).children[0];
        let field2 = find_field(&result2.cst, section2, "exposed-modules").unwrap();
        let edits2 = remove_list_item(&result2.cst, field2, "Data.List");
        let removed_src = apply_edits(&added_src, edits2);

        assert_eq!(
            removed_src, src,
            "round-trip add+remove should restore original"
        );
    }

    #[test]
    fn round_trip_add_remove_trailing_comma() {
        let src = "\
library
  build-depends:
    base >=4.14,
    aeson ^>=2.2
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();

        // Add text in sorted order.
        let edits = add_list_item(&result.cst, field, "text >=2.0", true);
        let added_src = apply_edits(src, edits);
        assert!(added_src.contains("text >=2.0"));

        // Remove.
        let result2 = parse::parse(&added_src);
        let section2 = result2.cst.node(result2.cst.root).children[0];
        let field2 = find_field(&result2.cst, section2, "build-depends").unwrap();
        let edits2 = remove_list_item(&result2.cst, field2, "text");
        let removed_src = apply_edits(&added_src, edits2);

        assert_eq!(
            removed_src, src,
            "round-trip add+remove should restore original"
        );
    }

    // -- item_name helper tests -------------------------------------------

    #[test]
    fn item_name_basic() {
        assert_eq!(item_name("base >=4.14"), "base");
        assert_eq!(item_name("aeson ^>=2.2"), "aeson");
        assert_eq!(item_name("  , text >=2.0"), "text");
        assert_eq!(item_name("Data.Map"), "Data.Map");
        assert_eq!(item_name("base,"), "base");
    }

    // -- add_list_item to empty field tests --------------------------------

    #[test]
    fn add_to_empty_field_single_line() {
        let src = "\
library
  build-depends:
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "build-depends").unwrap();
        let edits = add_list_item(&result.cst, field, "base >=4.14", false);
        let new_src = apply_edits(src, edits);

        assert!(new_src.contains("base >=4.14"));
        let re_parsed = parse::parse(&new_src);
        assert_eq!(re_parsed.cst.render(), new_src);
    }

    // -- add_list_item sorted insertion tests ------------------------------

    #[test]
    fn add_list_item_sorted_beginning() {
        let src = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
        let result = parse::parse(src);
        let section = result.cst.node(result.cst.root).children[0];
        let field = find_field(&result.cst, section, "exposed-modules").unwrap();
        let edits = add_list_item(&result.cst, field, "Data.Aeson", true);
        let new_src = apply_edits(src, edits);

        // Data.Aeson should come before Data.Map.
        let aeson_pos = new_src.find("Data.Aeson").unwrap();
        let map_pos = new_src.find("Data.Map").unwrap();
        assert!(aeson_pos < map_pos);
    }
}
