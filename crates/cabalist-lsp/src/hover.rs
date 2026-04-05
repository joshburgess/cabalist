//! Hover information providers for `.cabal` files.
//!
//! Returns contextual documentation when hovering over extension names,
//! warning flags, package names, and field names.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;
use crate::state::DocumentState;

/// Static documentation for well-known `.cabal` field names.
const FIELD_DOCS: &[(&str, &str)] = &[
    ("cabal-version", "Minimum version of the Cabal specification required to parse this file. Should be the first field."),
    ("name", "The package name. Must match the `.cabal` filename."),
    ("version", "The package version, following the Package Versioning Policy (PVP): A.B.C.D."),
    ("synopsis", "A one-line summary of the package, shown on Hackage search results."),
    ("description", "A longer description of the package, shown on the Hackage package page. Supports Haddock markup."),
    ("license", "The SPDX license identifier for the package."),
    ("license-file", "Path to the license file."),
    ("author", "The original author(s) of the package."),
    ("maintainer", "The current maintainer(s) — typically an email address."),
    ("homepage", "URL of the package homepage."),
    ("bug-reports", "URL where users should report bugs."),
    ("category", "A Hackage category for the package (e.g., 'Data', 'Web', 'Testing')."),
    ("build-type", "How to build the package: Simple (most common), Configure, Make, or Custom."),
    ("tested-with", "GHC versions the package is tested against (e.g., 'GHC ==9.8.2, GHC ==9.6.4')."),
    ("build-depends", "Library dependencies with version constraints. Use `^>=` for PVP-compliant bounds."),
    ("exposed-modules", "Modules visible to consumers of this library."),
    ("other-modules", "Internal modules not exposed to consumers."),
    ("hs-source-dirs", "Directories containing Haskell source files (default: current directory)."),
    ("default-language", "The base language standard: Haskell2010, Haskell98, GHC2021, or GHC2024."),
    ("default-extensions", "Language extensions enabled for all modules in this component."),
    ("ghc-options", "GHC command-line flags for this component (e.g., warning flags)."),
    ("main-is", "The entry-point module file for an executable or test-suite."),
    ("import", "Import fields from a `common` stanza."),
    ("extra-source-files", "Files to include in the source distribution but not install."),
];

/// Compute hover information for the given position.
pub fn hover(
    doc: &DocumentState,
    position: Position,
    hackage: Option<&cabalist_hackage::HackageIndex>,
) -> Option<Hover> {
    let offset = doc.line_index.position_to_offset(position);
    let source = &doc.source;

    // Find the current line.
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    let line_text = &source[line_start..line_end];
    let trimmed = line_text.trim_start();

    // Determine what the cursor is on.

    // 1. Check if hovering over a field name (before the colon).
    if let Some(colon_pos) = trimmed.find(':') {
        let field_name_part = &trimmed[..colon_pos];
        if !field_name_part.contains(' ') && !field_name_part.is_empty() {
            let indent = line_text.len() - trimmed.len();
            let field_name_start = line_start + indent;
            let field_name_end = field_name_start + colon_pos;

            // Is the cursor within the field name?
            if offset >= field_name_start && offset <= field_name_end {
                let canonical = field_name_part.to_ascii_lowercase().replace('_', "-");
                return hover_field_name(&canonical, &doc.line_index, field_name_start, field_name_end);
            }
        }
    }

    // 2. Check if cursor is in a field value — find the parent field.
    let indent = line_text.len() - trimmed.len();
    let field_name = if let Some(colon_pos) = trimmed.find(':') {
        let name_part = &trimmed[..colon_pos];
        if !name_part.contains(' ') && !name_part.is_empty() {
            Some(name_part.to_ascii_lowercase().replace('_', "-"))
        } else {
            None
        }
    } else if indent > 0 {
        // Continuation line — look backwards for the field name.
        find_parent_field(source, line_start)
    } else {
        None
    };

    let field_name = field_name?;

    // Find the word under the cursor.
    let word = word_at_offset(source, offset)?;

    match field_name.as_str() {
        "default-extensions" | "other-extensions" => hover_extension(&word, &doc.line_index, offset, &word),
        "ghc-options" | "ghc-prof-options" => hover_warning(&word, &doc.line_index, offset, &word),
        "build-depends" | "build-tool-depends" => hover_package(&word, &doc.line_index, offset, &word, hackage),
        _ => None,
    }
}

fn hover_field_name(
    field_name: &str,
    line_index: &LineIndex,
    start: usize,
    end: usize,
) -> Option<Hover> {
    let doc = FIELD_DOCS.iter().find(|(name, _)| *name == field_name)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("**{}**\n\n{}", field_name, doc.1),
        }),
        range: Some(line_index.span_to_range(cabalist_parser::span::Span::new(start, end))),
    })
}

fn hover_extension(
    ext_name: &str,
    line_index: &LineIndex,
    offset: usize,
    word: &str,
) -> Option<Hover> {
    let ext = cabalist_ghc::extensions::extension_info(ext_name)?;
    let mut doc = format!("**{}**\n\n{}", ext.name, ext.description);
    doc.push_str(&format!("\n\n*Since GHC {}*", ext.since));
    doc.push_str(&format!(" | Category: {}", ext.category));
    if ext.safe {
        doc.push_str(" | Safe to enable globally");
    }
    if ext.recommended {
        doc.push_str("\n\n*Recommended by cabalist*");
    }
    if let Some(ref warn) = ext.warn {
        doc.push_str(&format!("\n\n**Warning:** {warn}"));
    }

    // Compute the range of the word for highlighting.
    let word_start = offset.saturating_sub(word.len());
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc,
        }),
        range: Some(line_index.span_to_range(cabalist_parser::span::Span::new(
            word_start,
            word_start + word.len(),
        ))),
    })
}

fn hover_warning(
    flag: &str,
    line_index: &LineIndex,
    offset: usize,
    word: &str,
) -> Option<Hover> {
    let w = cabalist_ghc::warnings::warning_info(flag)?;
    let mut doc = format!("**{}**\n\n{}", w.flag, w.description);
    doc.push_str(&format!("\n\n*Since GHC {}*", w.since));
    if !w.group.is_empty() {
        doc.push_str(&format!("\n\nIncluded in: {}", w.group.join(", ")));
    }

    let word_start = offset.saturating_sub(word.len());
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc,
        }),
        range: Some(line_index.span_to_range(cabalist_parser::span::Span::new(
            word_start,
            word_start + word.len(),
        ))),
    })
}

fn hover_package(
    pkg_name: &str,
    line_index: &LineIndex,
    offset: usize,
    word: &str,
    hackage: Option<&cabalist_hackage::HackageIndex>,
) -> Option<Hover> {
    let index = hackage?;
    let info = index.package_info(pkg_name)?;
    let mut doc = format!("**{}**\n\n{}", info.name, info.synopsis);
    if let Some(latest) = info.latest_version() {
        doc.push_str(&format!("\n\nLatest version: **{}**", latest));
        let bounds = cabalist_hackage::compute_pvp_bounds(latest);
        doc.push_str(&format!("\nPVP bounds: `{bounds}`"));
    }
    let version_count = info.versions.len();
    doc.push_str(&format!("\n\n{version_count} version(s) on Hackage"));

    let word_start = offset.saturating_sub(word.len());
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc,
        }),
        range: Some(line_index.span_to_range(cabalist_parser::span::Span::new(
            word_start,
            word_start + word.len(),
        ))),
    })
}

/// Find the word (contiguous non-whitespace, non-comma) at the given byte offset.
fn word_at_offset(source: &str, offset: usize) -> Option<String> {
    if offset >= source.len() {
        return None;
    }

    let bytes = source.as_bytes();
    let is_word_char = |b: u8| !b.is_ascii_whitespace() && b != b',' && b != b'(' && b != b')';

    // If the cursor is not on a word character, no word here.
    if !is_word_char(bytes[offset]) {
        return None;
    }

    // Find word start.
    let mut start = offset;
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }

    // Find word end.
    let mut end = offset;
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(source[start..end].to_string())
}

/// Look backwards from a continuation line to find the parent field name.
fn find_parent_field(source: &str, from: usize) -> Option<String> {
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
        if indent == 0 && !trimmed.is_empty() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DocumentState;
    use tower_lsp::lsp_types::Position;

    fn make_doc(source: &str) -> DocumentState {
        DocumentState::new(source.to_string(), 1)
    }

    /// Helper: get hover content at a byte offset.
    fn hover_at(source: &str, offset: usize) -> Option<String> {
        let doc = make_doc(source);
        let pos = doc.line_index.offset_to_position(offset);
        let result = hover(&doc, pos, None);
        result.map(|h| match h.contents {
            HoverContents::Markup(m) => m.value,
            _ => String::new(),
        })
    }

    #[test]
    fn hover_on_field_name() {
        let source = "name: foo\n";
        // Hover on "name" (offset 0-4).
        let content = hover_at(source, 2);
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("**name**"));
        assert!(text.contains("package name"));
    }

    #[test]
    fn hover_on_build_depends_field() {
        let source = "library\n  build-depends: base\n";
        // Hover on "build-depends" field name.
        let offset = source.find("build-depends").unwrap() + 3;
        let content = hover_at(source, offset);
        assert!(content.is_some());
        assert!(content.unwrap().contains("**build-depends**"));
    }

    #[test]
    fn hover_on_extension_name() {
        let source = "library\n  default-extensions: OverloadedStrings\n";
        let offset = source.find("OverloadedStrings").unwrap() + 5;
        let content = hover_at(source, offset);
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("**OverloadedStrings**"));
        assert!(text.contains("Since GHC"));
    }

    #[test]
    fn hover_on_warning_flag() {
        let source = "library\n  ghc-options: -Wall\n";
        let offset = source.find("-Wall").unwrap() + 2;
        let content = hover_at(source, offset);
        assert!(content.is_some());
        let text = content.unwrap();
        assert!(text.contains("**-Wall**"));
    }

    #[test]
    fn hover_on_extension_continuation_line() {
        let source = "library\n  default-extensions:\n    OverloadedStrings\n    DerivingStrategies\n";
        let offset = source.find("DerivingStrategies").unwrap() + 3;
        let content = hover_at(source, offset);
        assert!(content.is_some());
        assert!(content.unwrap().contains("**DerivingStrategies**"));
    }

    #[test]
    fn hover_on_unknown_extension_returns_none() {
        let source = "library\n  default-extensions: NotARealExtension\n";
        let offset = source.find("NotARealExtension").unwrap() + 3;
        let content = hover_at(source, offset);
        assert!(content.is_none());
    }

    #[test]
    fn hover_on_empty_space_returns_none() {
        let source = "name: foo\n\n";
        let content = hover_at(source, 10); // the blank line
        assert!(content.is_none());
    }

    #[test]
    fn word_at_offset_basic() {
        assert_eq!(word_at_offset("hello world", 3), Some("hello".into()));
        assert_eq!(word_at_offset("hello world", 7), Some("world".into()));
        assert_eq!(word_at_offset("hello world", 5), None); // on the space
    }

    #[test]
    fn word_at_offset_with_commas() {
        assert_eq!(word_at_offset("base, text", 1), Some("base".into()));
        assert_eq!(word_at_offset("base, text", 6), Some("text".into()));
    }

    #[test]
    fn find_parent_field_basic() {
        let source = "library\n  build-depends:\n    base\n    ";
        let from = source.rfind("    ").unwrap();
        assert_eq!(find_parent_field(source, from), Some("build-depends".into()));
    }

    #[test]
    fn find_parent_field_stops_at_section() {
        let source = "library\n  exposed-modules: Foo\n";
        let from = source.len();
        // Looking from end, should find exposed-modules.
        assert_eq!(find_parent_field(source, from), Some("exposed-modules".into()));
    }
}
