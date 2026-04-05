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
            let pkg_name = extract_package_name(&doc.source);
            let synopsis_text = match &pkg_name {
                Some(name) => format!("synopsis: {name}\n"),
                None => "synopsis: A short description of the package\n".to_string(),
            };
            Some(make_edit_action(
                "Add `synopsis` field",
                uri,
                insert_pos,
                &synopsis_text,
                diag,
            ))
        }
        "missing-description" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            let pkg_name = extract_package_name(&doc.source);
            let desc_text = match &pkg_name {
                Some(name) => format!("description: Please see the README for {name}\n"),
                None => "description: Please see the README\n".to_string(),
            };
            Some(make_edit_action(
                "Add `description` field",
                uri,
                insert_pos,
                &desc_text,
                diag,
            ))
        }
        "missing-bug-reports" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            let bug_url = extract_repo_url(&doc.source)
                .map(|repo| format!("{repo}/issues"))
                .unwrap_or_else(|| "https://github.com/OWNER/REPO/issues".to_string());
            Some(make_edit_action(
                "Add `bug-reports` field",
                uri,
                insert_pos,
                &format!("bug-reports: {bug_url}\n"),
                diag,
            ))
        }
        "missing-source-repo" => {
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            let repo_url = extract_homepage(&doc.source)
                .unwrap_or_else(|| "https://github.com/OWNER/REPO".to_string());
            let text = format!(
                "\nsource-repository head\n  type: git\n  location: {repo_url}.git\n"
            );
            Some(make_edit_action(
                "Add `source-repository` section",
                uri,
                insert_pos,
                &text,
                diag,
            ))
        }
        "missing-upper-bound" => {
            // Use structured data from diagnostic if available, fall back to message parsing.
            let (pkg_name, constraint) = extract_dep_data(diag);
            let pkg_name = pkg_name?;

            // Extract the lower bound version from the constraint (e.g., ">=4.17" -> "4.17").
            let lower_version = constraint
                .as_deref()
                .and_then(|c| c.strip_prefix(">="))
                .map(|v| v.trim().to_string());

            lower_version.map(|version| make_replace_action(
                    &format!("Add PVP upper bound: ^>={version}"),
                    uri,
                    diag.range,
                    &format!("{pkg_name} ^>={version}"),
                    diag,
                ))
        }
        "duplicate-dep" => {
            // The diagnostic range points to the duplicate dependency line.
            // The fix is to remove it (replace with empty string).
            Some(make_replace_action(
                "Remove duplicate dependency",
                uri,
                Range {
                    start: Position {
                        line: diag.range.start.line,
                        character: 0,
                    },
                    end: Position {
                        line: diag.range.end.line + 1,
                        character: 0,
                    },
                },
                "",
                diag,
            ))
        }
        "missing-lower-bound" => {
            let (pkg_name, constraint) = extract_dep_data(diag);
            let pkg_name = pkg_name?;

            // Extract version from constraint (e.g., "<5" -> "5").
            let upper_version = constraint
                .as_deref()
                .and_then(|c| c.strip_prefix("<"))
                .map(|v| v.trim().to_string());

            upper_version.map(|version| make_replace_action(
                    &format!("Add PVP bounds: ^>={version}"),
                    uri,
                    diag.range,
                    &format!("{pkg_name} ^>={version}"),
                    diag,
                ))
        }
        "wide-any-version" => {
            let (pkg_name, _) = extract_dep_data(diag);
            let pkg_name = pkg_name?;
            Some(make_replace_action(
                &format!("Add placeholder bounds for '{pkg_name}'"),
                uri,
                diag.range,
                &format!("{pkg_name} ^>=0.1"),
                diag,
            ))
        }
        "ghc-options-werror" => {
            // The fix: remove -Werror from ghc-options. Since we can't easily edit
            // a single item in a ghc-options list via ranges, offer to wrap it in a flag.
            // For simplicity, just offer to remove the line containing -Werror.
            None // Too complex for a simple text replacement — skip for now.
        }
        "exposed-no-modules" => {
            // Insert an exposed-modules field after the section header.
            let insert_pos = Position {
                line: diag.range.start.line + 1,
                character: 0,
            };
            Some(make_edit_action(
                "Add `exposed-modules` field",
                uri,
                insert_pos,
                "  exposed-modules: MyModule\n",
                diag,
            ))
        }
        "unused-flag" => {
            // Remove the entire flag section (from the flag line to the next section).
            Some(make_replace_action(
                "Remove unused flag",
                uri,
                Range {
                    start: Position {
                        line: diag.range.start.line,
                        character: 0,
                    },
                    end: Position {
                        line: diag.range.end.line + 1,
                        character: 0,
                    },
                },
                "",
                diag,
            ))
        }
        "stale-tested-with" => {
            // Remove the stale tested-with entry. For simplicity, clear the field.
            let result = cabalist_parser::parse(&doc.source);
            if let Some(field_id) =
                cabalist_parser::edit::find_field(&result.cst, result.cst.root, "tested-with")
            {
                let node = &result.cst.nodes[field_id.0];
                let range = doc.line_index.span_to_range(node.span);
                Some(make_replace_action(
                    "Remove stale `tested-with` field",
                    uri,
                    range,
                    "",
                    diag,
                ))
            } else {
                None
            }
        }
        "no-common-stanza" => {
            // Insert a common stanza template before the first section.
            let insert_pos = find_top_level_insert_position(&doc.source, &doc.line_index);
            Some(make_edit_action(
                "Add a `common` stanza",
                uri,
                insert_pos,
                "\ncommon warnings\n  ghc-options: -Wall\n  default-language: GHC2021\n\n",
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

/// Extract package name and constraint from diagnostic structured data.
///
/// Reads from `diag.data` (a JSON object with "package" and "constraint" keys)
/// when available. Falls back to parsing the message string.
fn extract_dep_data(diag: &Diagnostic) -> (Option<String>, Option<String>) {
    // Try structured data first.
    if let Some(ref data) = diag.data {
        let pkg = data.get("package").and_then(|v| v.as_str()).map(|s| s.to_string());
        let constraint = data.get("constraint").and_then(|v| v.as_str()).map(|s| s.to_string());
        if pkg.is_some() {
            return (pkg, constraint);
        }
    }
    // Fall back to message parsing.
    let pkg = extract_quoted_name(&diag.message);
    let constraint = diag.message
        .find('(')
        .and_then(|start| {
            let rest = &diag.message[start + 1..];
            rest.find(')').map(|end| rest[..end].trim().to_string())
        });
    (pkg, constraint)
}

/// Extract a single-quoted name from a diagnostic message (e.g., "Dependency 'pkg' ...").
fn extract_quoted_name(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Extract the package name from a `.cabal` source for use in generated text.
fn extract_package_name(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("name:") {
            let value = trimmed["name:".len()..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract the homepage URL from a `.cabal` source.
fn extract_homepage(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("homepage:") {
            let value = trimmed["homepage:".len()..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract the source-repository URL from a `.cabal` source for deriving bug-reports.
fn extract_repo_url(source: &str) -> Option<String> {
    // Try homepage first (often a GitHub URL).
    if let Some(url) = extract_homepage(source) {
        if url.contains("github.com") || url.contains("gitlab.com") {
            return Some(url.trim_end_matches(".git").to_string());
        }
    }
    None
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

/// Check if a code action would be generated for a given lint ID.
#[cfg(test)]
fn has_action_for_lint(lint_id: &str) -> bool {
    let source = "cabal-version: 2.4\nname: test-pkg\nversion: 0.1\nhomepage: https://github.com/user/test-pkg\n\nlibrary\n  exposed-modules: Lib\n  build-depends: base ^>=4.17\n";
    let doc = crate::state::DocumentState::new(source.to_string(), 1);
    let uri = Url::parse("file:///test.cabal").unwrap();

    let diag = Diagnostic {
        range: Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 10 },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("cabalist".into()),
        code: Some(NumberOrString::String(lint_id.to_string())),
        message: "test".to_string(),
        ..Default::default()
    };

    action_for_lint(&doc, &uri, Path::new("."), &diag, lint_id).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_exists_for_missing_default_language() {
        assert!(has_action_for_lint("missing-default-language"));
    }

    #[test]
    fn action_exists_for_missing_synopsis() {
        assert!(has_action_for_lint("missing-synopsis"));
    }

    #[test]
    fn action_exists_for_missing_description() {
        assert!(has_action_for_lint("missing-description"));
    }

    #[test]
    fn action_exists_for_missing_bug_reports() {
        assert!(has_action_for_lint("missing-bug-reports"));
    }

    #[test]
    fn action_exists_for_cabal_version_low() {
        assert!(has_action_for_lint("cabal-version-low"));
    }

    #[test]
    fn action_exists_for_missing_source_repo() {
        assert!(has_action_for_lint("missing-source-repo"));
    }

    #[test]
    fn no_action_for_unknown_lint() {
        assert!(!has_action_for_lint("some-unknown-lint"));
    }

    #[test]
    fn synopsis_action_uses_package_name() {
        let source = "cabal-version: 3.0\nname: my-cool-lib\nversion: 0.1\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let diag = Diagnostic {
            range: Range::default(),
            source: Some("cabalist".into()),
            code: Some(NumberOrString::String("missing-synopsis".into())),
            message: "test".into(),
            ..Default::default()
        };
        let action = action_for_lint(&doc, &uri, Path::new("."), &diag, "missing-synopsis").unwrap();
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert!(
            edits[0].new_text.contains("my-cool-lib"),
            "synopsis should use the package name, got: {}",
            edits[0].new_text
        );
    }

    #[test]
    fn description_action_uses_package_name() {
        let source = "cabal-version: 3.0\nname: my-cool-lib\nversion: 0.1\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let diag = Diagnostic {
            range: Range::default(),
            source: Some("cabalist".into()),
            code: Some(NumberOrString::String("missing-description".into())),
            message: "test".into(),
            ..Default::default()
        };
        let action = action_for_lint(&doc, &uri, Path::new("."), &diag, "missing-description").unwrap();
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert!(
            edits[0].new_text.contains("my-cool-lib"),
            "description should reference the package name"
        );
    }

    #[test]
    fn action_exists_for_missing_upper_bound() {
        let source = "cabal-version: 3.0\nname: test-pkg\nversion: 0.1\nhomepage: https://github.com/user/test-pkg\n\nlibrary\n  exposed-modules: Lib\n  build-depends: base >=4.17\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let diag = Diagnostic {
            range: Range {
                start: Position { line: 7, character: 17 },
                end: Position { line: 7, character: 29 },
            },
            source: Some("cabalist".into()),
            code: Some(NumberOrString::String("missing-upper-bound".into())),
            message: "Dependency 'base' has no upper version bound (>=4.17). This violates the PVP.".into(),
            ..Default::default()
        };
        let action = action_for_lint(&doc, &uri, Path::new("."), &diag, "missing-upper-bound");
        assert!(action.is_some());
        let action = action.unwrap();
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert!(edits[0].new_text.contains("^>=4.17"), "should add PVP bound, got: {}", edits[0].new_text);
    }

    #[test]
    fn action_exists_for_duplicate_dep() {
        assert!(has_action_for_lint("duplicate-dep"));
    }

    #[test]
    fn bug_reports_action_uses_homepage() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\nhomepage: https://github.com/user/repo\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();
        let diag = Diagnostic {
            range: Range::default(),
            source: Some("cabalist".into()),
            code: Some(NumberOrString::String("missing-bug-reports".into())),
            message: "test".into(),
            ..Default::default()
        };
        let action = action_for_lint(&doc, &uri, Path::new("."), &diag, "missing-bug-reports").unwrap();
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert!(
            edits[0].new_text.contains("https://github.com/user/repo/issues"),
            "bug-reports should derive from homepage, got: {}",
            edits[0].new_text
        );
    }

    #[test]
    fn code_actions_filters_by_source() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();

        let non_cabalist_diag = Diagnostic {
            source: Some("other-tool".into()),
            code: Some(NumberOrString::String("missing-synopsis".into())),
            message: "test".into(),
            ..Default::default()
        };

        let context = CodeActionContext {
            diagnostics: vec![non_cabalist_diag],
            ..Default::default()
        };

        let range = Range::default();
        let actions = code_actions(&doc, &uri, Path::new("."), &range, &context);
        assert!(actions.is_empty(), "should ignore non-cabalist diagnostics");
    }

    #[test]
    fn cabal_version_low_action_has_workspace_edit() {
        let source = "cabal-version: 2.4\nname: test\nversion: 0.1\n";
        let doc = crate::state::DocumentState::new(source.to_string(), 1);
        let uri = Url::parse("file:///test.cabal").unwrap();

        let diag = Diagnostic {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 18 },
            },
            source: Some("cabalist".into()),
            code: Some(NumberOrString::String("cabal-version-low".into())),
            message: "cabal-version is 2.4".into(),
            ..Default::default()
        };

        let action = action_for_lint(&doc, &uri, Path::new("."), &diag, "cabal-version-low");
        assert!(action.is_some());
        let action = action.unwrap();
        assert!(action.edit.is_some(), "should have a workspace edit");
        let edit = action.edit.unwrap();
        let changes = edit.changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert!(!edits.is_empty());
        assert!(
            edits[0].new_text.contains("3.0"),
            "replacement should contain 3.0"
        );
    }
}
