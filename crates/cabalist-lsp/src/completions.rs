//! Completion providers for `.cabal` files.
//!
//! Context-sensitive completions based on cursor position: field names,
//! extension names, warning flags, package names, section keywords, and
//! static value lists.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;
use crate::state::DocumentState;

/// Top-level metadata field names.
const TOP_LEVEL_FIELDS: &[&str] = &[
    "cabal-version",
    "name",
    "version",
    "synopsis",
    "description",
    "license",
    "license-file",
    "author",
    "maintainer",
    "homepage",
    "bug-reports",
    "category",
    "build-type",
    "tested-with",
    "extra-source-files",
    "extra-doc-files",
    "data-files",
    "data-dir",
];

/// Field names valid inside a component section.
const SECTION_FIELDS: &[&str] = &[
    "build-depends",
    "exposed-modules",
    "other-modules",
    "hs-source-dirs",
    "default-language",
    "default-extensions",
    "other-extensions",
    "ghc-options",
    "ghc-prof-options",
    "import",
    "main-is",
    "type",
    "build-tool-depends",
    "mixins",
    "autogen-modules",
    "reexported-modules",
    "signatures",
    "cpp-options",
    "cc-options",
    "ld-options",
    "pkgconfig-depends",
    "frameworks",
    "extra-libraries",
    "extra-lib-dirs",
    "includes",
    "include-dirs",
    "c-sources",
    "js-sources",
];

/// Section header keywords.
const SECTION_KEYWORDS: &[&str] = &[
    "library",
    "executable",
    "test-suite",
    "benchmark",
    "common",
    "flag",
    "source-repository",
];

/// Common license identifiers.
const LICENSES: &[&str] = &[
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Apache-2.0",
    "ISC",
    "MPL-2.0",
    "GPL-2.0-only",
    "GPL-3.0-only",
    "LGPL-2.1-only",
    "LGPL-3.0-only",
    "AGPL-3.0-only",
    "0BSD",
    "Unlicense",
];

const LANGUAGES: &[&str] = &["GHC2021", "GHC2024", "Haskell2010", "Haskell98"];
const BUILD_TYPES: &[&str] = &["Simple", "Configure", "Make", "Custom"];
const TEST_TYPES: &[&str] = &["exitcode-stdio-1.0", "detailed-0.9"];
const CONDITION_FUNCS: &[&str] = &["flag(", "os(", "arch(", "impl("];

/// The context at the cursor position, determining what completions to offer.
enum CompletionContext {
    /// Cursor is at a position where a field name is expected (start of line).
    FieldName { in_section: bool },
    /// Cursor is inside the value of a known field.
    FieldValue { field_name: String },
    /// Cursor is at a position where a section header keyword is expected.
    SectionKeyword,
    /// Cursor is after an `if` keyword.
    Condition,
    /// Unknown context.
    Unknown,
}

/// Compute completions for the given position in a document.
pub fn completions(doc: &DocumentState, position: Position) -> Vec<CompletionItem> {
    let offset = doc.line_index.position_to_offset(position);
    let ctx = detect_context(&doc.source, offset);

    match ctx {
        CompletionContext::FieldName { in_section } => {
            let fields = if in_section {
                SECTION_FIELDS
            } else {
                TOP_LEVEL_FIELDS
            };
            fields
                .iter()
                .map(|&name| CompletionItem {
                    label: format!("{name}:"),
                    kind: Some(CompletionItemKind::FIELD),
                    insert_text: Some(format!("{name}: ")),
                    ..Default::default()
                })
                .collect()
        }
        CompletionContext::SectionKeyword => SECTION_KEYWORDS
            .iter()
            .map(|&kw| CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            })
            .collect(),
        CompletionContext::Condition => CONDITION_FUNCS
            .iter()
            .map(|&f| CompletionItem {
                label: f.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                ..Default::default()
            })
            .collect(),
        CompletionContext::FieldValue { field_name } => {
            complete_field_value(&field_name, &doc.source, &doc.line_index)
        }
        CompletionContext::Unknown => Vec::new(),
    }
}

/// Detect what completion context the cursor is in.
fn detect_context(source: &str, offset: usize) -> CompletionContext {
    // Find the current line.
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_text = &source[line_start..offset];
    let trimmed = line_text.trim_start();
    let indent = line_text.len() - trimmed.len();

    // If the line is empty or only whitespace, offer field names or section keywords.
    if trimmed.is_empty() {
        if indent == 0 {
            // Top-level: could be a field name or section keyword.
            // Offer both, but section keywords are more likely.
            return CompletionContext::SectionKeyword;
        } else {
            // Inside a section.
            return CompletionContext::FieldName { in_section: true };
        }
    }

    // If the line starts with "if " or "if\t", offer condition functions.
    if trimmed.starts_with("if ") || trimmed.starts_with("if\t") || trimmed == "if" {
        return CompletionContext::Condition;
    }

    // If the cursor is after a colon on the same line, we're in a field value.
    if let Some(colon_pos) = trimmed.find(':') {
        let field_name_part = &trimmed[..colon_pos];
        // Ensure this looks like a field name (no spaces before colon).
        if !field_name_part.contains(' ') && !field_name_part.is_empty() {
            let field_name = field_name_part.to_ascii_lowercase().replace('_', "-");
            return CompletionContext::FieldValue { field_name };
        }
    }

    // Check if this is a continuation line (indented, inside a multi-line field value).
    // Look backwards to find the field name.
    if indent > 0 {
        if let Some(field_name) = find_parent_field_name(source, line_start) {
            return CompletionContext::FieldValue { field_name };
        }
        return CompletionContext::FieldName { in_section: true };
    }

    // At column 0 with text: could be typing a field name or section keyword.
    if !trimmed.contains(':') {
        return CompletionContext::SectionKeyword;
    }

    CompletionContext::FieldName { in_section: false }
}

/// Look backwards from a continuation line to find the parent field name.
fn find_parent_field_name(source: &str, from: usize) -> Option<String> {
    let before = &source[..from];
    for line in before.lines().rev() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(colon_pos) = trimmed.find(':') {
            let field_name_part = &trimmed[..colon_pos];
            if !field_name_part.contains(' ') && !field_name_part.is_empty() {
                return Some(field_name_part.to_ascii_lowercase().replace('_', "-"));
            }
        }
        // If we hit a line with less indentation that doesn't have a colon,
        // it's a section header — stop looking.
        if indent == 0 && !trimmed.is_empty() {
            break;
        }
    }
    None
}

/// Generate completions for a specific field's value.
fn complete_field_value(
    field_name: &str,
    _source: &str,
    _line_index: &LineIndex,
) -> Vec<CompletionItem> {
    match field_name {
        "default-language" => static_completions(LANGUAGES, CompletionItemKind::ENUM_MEMBER),
        "build-type" => static_completions(BUILD_TYPES, CompletionItemKind::ENUM_MEMBER),
        "license" => static_completions(LICENSES, CompletionItemKind::ENUM_MEMBER),
        "type" => static_completions(TEST_TYPES, CompletionItemKind::ENUM_MEMBER),
        "default-extensions" | "other-extensions" => extension_completions(),
        "ghc-options" | "ghc-prof-options" => warning_completions(),
        // build-depends completions would use the hackage index — added later.
        _ => Vec::new(),
    }
}

fn static_completions(items: &[&str], kind: CompletionItemKind) -> Vec<CompletionItem> {
    items
        .iter()
        .map(|&item| CompletionItem {
            label: item.to_string(),
            kind: Some(kind),
            ..Default::default()
        })
        .collect()
}

fn extension_completions() -> Vec<CompletionItem> {
    cabalist_ghc::extensions::load_extensions()
        .iter()
        .map(|ext| {
            let mut item = CompletionItem {
                label: ext.name.clone(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(format!("Since GHC {}", ext.since)),
                ..Default::default()
            };
            let mut doc = ext.description.clone();
            if ext.recommended {
                doc.push_str("\n\n*Recommended by cabalist*");
            }
            if let Some(ref warn) = ext.warn {
                doc.push_str(&format!("\n\n**Warning:** {warn}"));
            }
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: doc,
            }));
            item
        })
        .collect()
}

fn warning_completions() -> Vec<CompletionItem> {
    cabalist_ghc::warnings::load_warnings()
        .iter()
        .map(|w| CompletionItem {
            label: w.flag.clone(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            detail: Some(format!("Since GHC {}", w.since)),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: w.description.clone(),
            })),
            ..Default::default()
        })
        .collect()
}
