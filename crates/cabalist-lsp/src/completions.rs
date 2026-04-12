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
}

/// Compute completions for the given position in a document.
pub fn completions(
    doc: &DocumentState,
    position: Position,
    hackage: Option<&cabalist_hackage::HackageIndex>,
) -> Vec<CompletionItem> {
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
            complete_field_value(&field_name, &doc.source, &doc.line_index, hackage)
        }
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
            return CompletionContext::SectionKeyword;
        } else {
            // Check if this indented empty line is a continuation of a field value.
            if let Some(field_name) = find_parent_field_name(source, line_start) {
                return CompletionContext::FieldValue { field_name };
            }
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
    source: &str,
    line_index: &LineIndex,
    hackage: Option<&cabalist_hackage::HackageIndex>,
) -> Vec<CompletionItem> {
    let _ = (source, line_index); // reserved for future use
    match field_name {
        "default-language" => static_completions(LANGUAGES, CompletionItemKind::ENUM_MEMBER),
        "build-type" => static_completions(BUILD_TYPES, CompletionItemKind::ENUM_MEMBER),
        "license" => static_completions(LICENSES, CompletionItemKind::ENUM_MEMBER),
        "type" => static_completions(TEST_TYPES, CompletionItemKind::ENUM_MEMBER),
        "default-extensions" | "other-extensions" => extension_completions(),
        "ghc-options" | "ghc-prof-options" => warning_completions(),
        "build-depends" | "build-tool-depends" => package_completions(hackage),
        _ => Vec::new(),
    }
}

fn package_completions(hackage: Option<&cabalist_hackage::HackageIndex>) -> Vec<CompletionItem> {
    let Some(index) = hackage else {
        return Vec::new();
    };

    // Return a subset of popular packages as default completions.
    // Full search happens as the user types (via trigger characters).
    // We show a curated set when the field is first entered.
    let popular = [
        "base",
        "text",
        "bytestring",
        "containers",
        "aeson",
        "mtl",
        "transformers",
        "vector",
        "unordered-containers",
        "hashable",
        "filepath",
        "directory",
        "process",
        "time",
        "stm",
        "async",
        "http-client",
        "http-types",
        "warp",
        "servant",
        "optparse-applicative",
        "tasty",
        "hspec",
        "QuickCheck",
        "criterion",
    ];

    popular
        .iter()
        .filter_map(|&name| {
            let info = index.package_info(name)?;
            let latest = info.latest_version()?;
            let bounds = cabalist_hackage::compute_pvp_bounds(latest);
            Some(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail: Some(format!("{} (latest: {})", info.synopsis, latest)),
                insert_text: Some(format!("{name} {bounds}")),
                sort_text: Some(format!("0_{name}")), // sort before non-popular
                ..Default::default()
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DocumentState;

    fn make_doc(source: &str) -> DocumentState {
        DocumentState::new(source.to_string(), 1)
    }

    /// Helper: get completion labels at a byte offset in the source.
    fn labels_at(source: &str, offset: usize) -> Vec<String> {
        let doc = make_doc(source);
        let pos = doc.line_index.offset_to_position(offset);
        let items = completions(&doc, pos, None);
        items.into_iter().map(|i| i.label).collect()
    }

    #[test]
    fn top_level_empty_line_offers_section_keywords() {
        let source = "cabal-version: 3.0\nname: foo\n\n";
        // Cursor at the empty line (offset 30 = start of blank line).
        let labels = labels_at(source, 30);
        assert!(labels.contains(&"library".to_string()));
        assert!(labels.contains(&"executable".to_string()));
    }

    #[test]
    fn indented_empty_line_offers_section_fields() {
        let source = "cabal-version: 3.0\n\nlibrary\n  \n";
        // Cursor at the indented empty line inside the library section.
        let offset = source.rfind("  \n").unwrap() + 2;
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"build-depends:".to_string()));
        assert!(labels.contains(&"exposed-modules:".to_string()));
    }

    #[test]
    fn default_extensions_value_offers_ghc_extensions() {
        let source = "library\n  default-extensions: ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"OverloadedStrings".to_string()));
        assert!(labels.contains(&"DerivingStrategies".to_string()));
    }

    #[test]
    fn ghc_options_value_offers_warning_flags() {
        let source = "library\n  ghc-options: ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.iter().any(|l| l.starts_with("-W")));
    }

    #[test]
    fn default_language_value_offers_languages() {
        let source = "library\n  default-language: ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"GHC2021".to_string()));
        assert!(labels.contains(&"Haskell2010".to_string()));
    }

    #[test]
    fn license_value_offers_spdx() {
        let source = "license: ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"MIT".to_string()));
        assert!(labels.contains(&"BSD-3-Clause".to_string()));
    }

    #[test]
    fn build_type_value_offers_types() {
        let source = "build-type: ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"Simple".to_string()));
        assert!(labels.contains(&"Custom".to_string()));
    }

    #[test]
    fn after_if_keyword_offers_conditions() {
        let source = "library\n  if ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        assert!(labels.contains(&"flag(".to_string()));
        assert!(labels.contains(&"os(".to_string()));
    }

    #[test]
    fn continuation_line_inherits_parent_field() {
        let source = "library\n  default-extensions:\n    OverloadedStrings\n    ";
        let offset = source.len();
        let labels = labels_at(source, offset);
        // Should offer extensions, not field names.
        assert!(labels.contains(&"DerivingStrategies".to_string()));
    }

    #[test]
    fn extension_completions_have_documentation() {
        let items = extension_completions();
        let overloaded = items.iter().find(|i| i.label == "OverloadedStrings");
        assert!(overloaded.is_some());
        assert!(overloaded.unwrap().detail.is_some());
        assert!(overloaded.unwrap().documentation.is_some());
    }

    #[test]
    fn context_detection_field_value_after_colon() {
        let source = "name: ";
        let ctx = detect_context(source, 6);
        assert!(
            matches!(ctx, CompletionContext::FieldValue { field_name } if field_name == "name")
        );
    }

    #[test]
    fn context_detection_section_keyword_at_col0() {
        let source = "lib";
        let ctx = detect_context(source, 3);
        assert!(matches!(ctx, CompletionContext::SectionKeyword));
    }
}
