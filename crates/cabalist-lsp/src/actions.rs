//! Code actions (quick fixes) generated from lint diagnostics.
//!
//! When a lint has a `suggestion` field, we generate a code action that
//! applies the suggested fix via workspace edits.

use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::*;

use crate::state::DocumentState;

/// Generate code actions for the given range and diagnostic context.
pub fn code_actions(
    doc: &DocumentState,
    uri: &Url,
    project_root: &Path,
    _range: &Range,
    context: &CodeActionContext,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for diag in &context.diagnostics {
        if diag.source.as_deref() != Some("cabalist") {
            continue;
        }
        let Some(ref code) = diag.code else {
            continue;
        };
        let NumberOrString::String(lint_id) = code else {
            continue;
        };

        if let Some(action) = action_for_lint(doc, uri, project_root, diag, lint_id) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
}

fn action_for_lint(
    doc: &DocumentState,
    uri: &Url,
    _project_root: &Path,
    diag: &Diagnostic,
    lint_id: &str,
) -> Option<CodeAction> {
    match lint_id {
        "missing-default-language" => {
            // Insert after the section header line (use diagnostic range end as hint).
            let insert_pos = Position {
                line: diag.range.start.line + 1,
                character: 0,
            };
            Some(make_edit_action(
                "Add `default-language: GHC2021`",
                uri,
                insert_pos,
                "  default-language: GHC2021\n",
                diag,
            ))
        }
        "missing-synopsis" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            Some(make_edit_action(
                "Add `synopsis` field",
                uri,
                insert_pos,
                "synopsis: TODO\n",
                diag,
            ))
        }
        "missing-description" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            Some(make_edit_action(
                "Add `description` field",
                uri,
                insert_pos,
                "description: TODO\n",
                diag,
            ))
        }
        "missing-bug-reports" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            Some(make_edit_action(
                "Add `bug-reports` field",
                uri,
                insert_pos,
                "bug-reports: https://github.com/OWNER/REPO/issues\n",
                diag,
            ))
        }
        "cabal-version-low" => {
            let result = cabalist_parser::parse(&doc.source);
            let field_id =
                cabalist_parser::edit::find_field(&result.cst, result.cst.root, "cabal-version")?;
            let edit = cabalist_parser::edit::set_field_value(&result.cst, field_id, "3.0");
            let range = doc.line_index.span_to_range(edit.range);

            Some(make_replace_action(
                "Upgrade cabal-version to 3.0",
                uri,
                range,
                &edit.replacement,
                diag,
            ))
        }
        _ => None,
    }
}

/// Find a good position to insert a new top-level field.
fn find_top_level_insert_position(
    source: &str,
    line_index: &crate::convert::LineIndex,
) -> Position {
    let result = cabalist_parser::parse(source);
    let root = &result.cst.nodes[result.cst.root.0];

    let mut insert_offset = 0usize;
    for &child_id in &root.children {
        let child = &result.cst.nodes[child_id.0];
        match child.kind {
            cabalist_parser::cst::CstNodeKind::Field
            | cabalist_parser::cst::CstNodeKind::Comment
            | cabalist_parser::cst::CstNodeKind::BlankLine => {
                insert_offset = child.span.end;
            }
            cabalist_parser::cst::CstNodeKind::Section => break,
            _ => {
                insert_offset = child.span.end;
            }
        }
    }

    line_index.offset_to_position(insert_offset)
}

fn make_edit_action(
    title: &str,
    uri: &Url,
    position: Position,
    text: &str,
    diag: &Diagnostic,
) -> CodeAction {
    let text_edit = TextEdit {
        range: Range {
            start: position,
            end: position,
        },
        new_text: text.to_string(),
    };

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![text_edit]);

    CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}

fn make_replace_action(
    title: &str,
    uri: &Url,
    range: Range,
    replacement: &str,
    diag: &Diagnostic,
) -> CodeAction {
    let text_edit = TextEdit {
        range,
        new_text: replacement.to_string(),
    };

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![text_edit]);

    CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }
}
