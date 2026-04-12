//! Semantic token provider for `.cabal` files.
//!
//! Provides richer syntax highlighting than TextMate grammars by distinguishing
//! field names, section keywords, version constraints, package names, extension
//! names, and warning flags with semantic types.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// The semantic token types used by cabalist-lsp.
/// Order must match [`LEGEND`].
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD, // 0: section keywords (library, executable, etc.)
    SemanticTokenType::PROPERTY, // 1: field names (build-depends, exposed-modules, etc.)
    SemanticTokenType::STRING,  // 2: field values
    SemanticTokenType::NUMBER,  // 3: version numbers
    SemanticTokenType::OPERATOR, // 4: version operators (^>=, >=, <, etc.)
    SemanticTokenType::NAMESPACE, // 5: package names in build-depends
    SemanticTokenType::ENUM_MEMBER, // 6: extension names, language names
    SemanticTokenType::COMMENT, // 7: comments
    SemanticTokenType::VARIABLE, // 8: flag/condition references
];

/// Build the semantic token legend.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: Vec::new(),
    }
}

/// Compute semantic tokens for a `.cabal` source file.
pub fn semantic_tokens(source: &str, line_index: &LineIndex) -> Vec<SemanticToken> {
    let result = cabalist_parser::parse(source);
    let cst = &result.cst;

    let mut raw_tokens: Vec<(u32, u32, u32, u32)> = Vec::new(); // (line, col, len, type)

    // Walk the CST nodes and classify them.
    for node in &cst.nodes {
        use cabalist_parser::cst::CstNodeKind;

        let start = line_index.offset_to_position(node.span.start);
        let text = &source[node.span.start..node.span.end];
        let length = text.len() as u32;

        if length == 0 {
            continue;
        }

        match node.kind {
            CstNodeKind::Comment => {
                raw_tokens.push((start.line, start.character, length, 7));
            }
            CstNodeKind::Section => {
                // The section keyword is the first word on the line.
                let first_line = text.lines().next().unwrap_or("");
                let keyword = first_line.split_whitespace().next().unwrap_or("");
                if !keyword.is_empty() {
                    let kw_len = keyword.len() as u32;
                    raw_tokens.push((start.line, start.character, kw_len, 0));

                    // Section name (if present).
                    if let Some(name_start) = first_line.find(keyword).map(|i| i + keyword.len()) {
                        let rest = first_line[name_start..].trim();
                        if !rest.is_empty() {
                            let name_col = start.character
                                + name_start as u32
                                + (first_line[name_start..].len()
                                    - first_line[name_start..].trim_start().len())
                                    as u32;
                            raw_tokens.push((start.line, name_col, rest.len() as u32, 2));
                        }
                    }
                }
            }
            CstNodeKind::Field => {
                // Field name is before the colon.
                if let Some(colon_pos) = text.find(':') {
                    let field_name = &text[..colon_pos];
                    let trimmed_name = field_name.trim();
                    if !trimmed_name.is_empty() {
                        let indent = field_name.len() - field_name.trim_start().len();
                        raw_tokens.push((
                            start.line,
                            start.character + indent as u32,
                            trimmed_name.len() as u32,
                            1,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Sort by position for delta encoding.
    raw_tokens.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Convert to delta-encoded SemanticTokens.
    let mut tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    for (line, col, length, token_type) in raw_tokens {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { col - prev_col } else { col };

        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = line;
        prev_col = col;
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_for_simple_file() {
        let source = "cabal-version: 3.0\nname: test\n\nlibrary\n  build-depends: base\n";
        let line_index = LineIndex::new(source);
        let tokens = semantic_tokens(source, &line_index);

        // Should produce tokens for field names and section keywords.
        assert!(!tokens.is_empty(), "should produce semantic tokens");
    }

    #[test]
    fn tokens_include_section_keywords() {
        let source = "cabal-version: 3.0\nname: test\n\nlibrary\n  build-depends: base\n";
        let line_index = LineIndex::new(source);
        let tokens = semantic_tokens(source, &line_index);

        // At least one token should be a keyword (type 0).
        let has_keyword = tokens.iter().any(|t| t.token_type == 0);
        assert!(has_keyword, "should have at least one keyword token");
    }

    #[test]
    fn tokens_include_field_names() {
        let source = "cabal-version: 3.0\nname: test\n\nlibrary\n  build-depends: base\n";
        let line_index = LineIndex::new(source);
        let tokens = semantic_tokens(source, &line_index);

        // At least one token should be a property (type 1).
        let has_property = tokens.iter().any(|t| t.token_type == 1);
        assert!(has_property, "should have at least one property token");
    }

    #[test]
    fn tokens_include_comments() {
        let source = "-- This is a comment\ncabal-version: 3.0\nname: test\n";
        let line_index = LineIndex::new(source);
        let tokens = semantic_tokens(source, &line_index);

        let has_comment = tokens.iter().any(|t| t.token_type == 7);
        assert!(has_comment, "should have a comment token");
    }

    #[test]
    fn empty_source_produces_no_tokens() {
        let source = "";
        let line_index = LineIndex::new(source);
        let tokens = semantic_tokens(source, &line_index);
        assert!(tokens.is_empty());
    }

    #[test]
    fn legend_has_correct_type_count() {
        let l = legend();
        assert_eq!(l.token_types.len(), TOKEN_TYPES.len());
    }
}
