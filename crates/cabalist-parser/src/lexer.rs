//! Hand-written lexer for `.cabal` files.
//!
//! The lexer operates line-by-line, classifying each line and producing tokens
//! that capture every byte of the input (via spans and trivia). The parser
//! consumes these tokens to build the CST.
//!
//! Key properties:
//! - Zero-copy: tokens are [`Span`] references into the source string.
//! - Full coverage: every byte is accounted for by a token span or trivia piece.
//! - Indentation tracking: each token records its column position (tabs = 8).

use crate::span::Span;

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

/// The kind of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// An identifier that appears before a `:` — a field name.
    FieldName,
    /// The `:` separator after a field name.
    Colon,
    /// A section keyword: `library`, `executable`, `test-suite`, `benchmark`,
    /// `flag`, `source-repository`, `common`.
    SectionHeader,
    /// The argument after a section header (e.g. the name in `executable foo`).
    SectionArg,
    /// The `if` keyword in a conditional.
    If,
    /// The `else` keyword.
    Else,
    /// The `elif` keyword (rare, but in the spec).
    Elif,
    /// Raw value text (the part after a colon on the same line, or a
    /// continuation line that is part of a field value).
    Value,
    /// A comma `,`.
    Comma,
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `!` (negation in conditions).
    Not,
    /// `&&`.
    And,
    /// `||`.
    Or,
    /// A comparison operator: `==`, `>=`, `<=`, `>`, `<`.
    CompOp,
    /// A line comment (starts with `--`). Stored as trivia on the *next*
    /// meaningful token, but also emitted as a standalone token when it is the
    /// only content on a line.
    Comment,
    /// End of file.
    Eof,
}

/// The kind of a trivia piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    /// Horizontal whitespace (spaces / tabs) within a line.
    Whitespace,
    /// A line-feed (`\n`) or carriage-return + line-feed (`\r\n`).
    Newline,
    /// A `--` comment (including the `--` prefix and everything to EOL,
    /// but *not* the newline itself).
    Comment,
}

/// A piece of trivia attached to a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriviaPiece {
    pub kind: TriviaKind,
    pub span: Span,
}

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte span of the meaningful content.
    pub span: Span,
    /// Column (0-based) of the first byte, with tabs expanded to multiples
    /// of 8.
    pub indent: usize,
    /// Trivia that precedes this token (whitespace, newlines, comments).
    pub leading_trivia: Vec<TriviaPiece>,
}

// ---------------------------------------------------------------------------
// Line classification (first pass)
// ---------------------------------------------------------------------------

/// High-level classification of a source line. The lexer first splits the
/// input into lines, classifies each, and then produces tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LineKind {
    /// A blank line (only whitespace).
    Blank,
    /// A comment line (leading whitespace + `--`).
    Comment,
    /// A section header keyword (e.g. `library`, `executable foo`).
    SectionHeader,
    /// A conditional keyword (`if`, `else`, `elif`).
    Conditional,
    /// A field: `name: value`.
    Field,
    /// A continuation / value line that doesn't match any of the above.
    Value,
}

/// Internal representation of one source line before tokenization.
#[derive(Debug, Clone)]
struct RawLine {
    /// Byte offset of the first character of the line in the source.
    start: usize,
    /// Byte offset one past the last character (before the newline, if any).
    end: usize,
    /// Byte offset of the newline(s) at the end (`\n` or `\r\n`). Equal to
    /// `end` if no newline (last line of file without trailing newline).
    newline_start: usize,
    /// Byte offset one past the newline (i.e. start of next line, or source
    /// len).
    line_end_with_newline: usize,
    /// Column of first non-whitespace character (tabs expanded to multiples
    /// of 8). `None` if the line is blank.
    indent: Option<usize>,
    /// Byte offset of the first non-whitespace character.
    content_start: usize,
    /// Classification.
    kind: LineKind,
}

// ---------------------------------------------------------------------------
// Section header keywords
// ---------------------------------------------------------------------------

const SECTION_KEYWORDS: &[&str] = &[
    "library",
    "executable",
    "test-suite",
    "benchmark",
    "flag",
    "source-repository",
    "common",
    "custom-setup",
    "foreign-library",
];

const CONDITIONAL_KEYWORDS: &[&str] = &["if", "else", "elif"];

/// Check whether `word` (already lowercased) is a section keyword.
fn is_section_keyword(word: &str) -> bool {
    SECTION_KEYWORDS
        .iter()
        .any(|kw| kw.eq_ignore_ascii_case(word))
}

fn is_conditional_keyword(word: &str) -> bool {
    CONDITIONAL_KEYWORDS
        .iter()
        .any(|kw| kw.eq_ignore_ascii_case(word))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the visual column for a run of bytes starting at column 0, where
/// tabs advance to the next multiple of 8.
fn visual_column(source: &[u8], start: usize, end: usize) -> usize {
    let mut col: usize = 0;
    for &b in &source[start..end] {
        if b == b'\t' {
            col = (col + 8) & !7; // next multiple of 8
        } else {
            col += 1;
        }
    }
    col
}

/// Extract the first "word" (letters, digits, hyphens, underscores) starting
/// at `pos` in `source`. Returns `(word_slice, end_offset)`.
fn scan_word(source: &[u8], pos: usize) -> (usize, usize) {
    let start = pos;
    let mut i = pos;
    while i < source.len()
        && (source[i].is_ascii_alphanumeric() || source[i] == b'-' || source[i] == b'_')
    {
        i += 1;
    }
    (start, i)
}

/// Skip horizontal whitespace (spaces and tabs) starting at `pos`.
fn skip_hspace(source: &[u8], pos: usize) -> usize {
    let mut i = pos;
    while i < source.len() && (source[i] == b' ' || source[i] == b'\t') {
        i += 1;
    }
    i
}

// ---------------------------------------------------------------------------
// Line splitting
// ---------------------------------------------------------------------------

/// Split `source` into `RawLine`s and classify each one.
fn split_lines(source: &str) -> Vec<RawLine> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut lines = Vec::new();
    let mut pos = 0;

    while pos <= len {
        let line_start = pos;

        // Find end of line content (before newline).
        let mut end = pos;
        while end < len && bytes[end] != b'\n' && bytes[end] != b'\r' {
            end += 1;
        }
        let content_end = end;

        // Consume newline.
        let newline_start = end;
        if end < len && bytes[end] == b'\r' {
            end += 1;
        }
        if end < len && bytes[end] == b'\n' {
            end += 1;
        }
        let line_end = end;

        // Find first non-whitespace.
        let mut first_non_ws = line_start;
        while first_non_ws < content_end
            && (bytes[first_non_ws] == b' ' || bytes[first_non_ws] == b'\t')
        {
            first_non_ws += 1;
        }

        let indent = if first_non_ws == content_end {
            None // blank line
        } else {
            Some(visual_column(bytes, line_start, first_non_ws))
        };

        let kind = classify_line(source, first_non_ws, content_end, indent.is_none());

        lines.push(RawLine {
            start: line_start,
            end: content_end,
            newline_start,
            line_end_with_newline: line_end,
            indent,
            content_start: first_non_ws,
            kind,
        });

        // Guard against infinite loop on last line without newline.
        if line_end == pos {
            break;
        }
        pos = line_end;
    }

    // Post-process: handle braced freeform text blocks.
    // When a Field line's value ends with `{`, all subsequent lines up to and
    // including a line that is just `}` are reclassified as Value lines so
    // the parser treats them as continuation values.
    reclassify_braced_freeform_blocks(&mut lines, source);

    lines
}

/// Detect braced freeform text blocks (e.g. `Description: { ... }`) and
/// reclassify contained lines as `Value` so the parser treats them as
/// field continuation lines.
fn reclassify_braced_freeform_blocks(lines: &mut [RawLine], source: &str) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < lines.len() {
        // Look for a Field line whose value part ends with `{`.
        if lines[i].kind == LineKind::Field {
            let line = &lines[i];
            // Check if the content (before newline) ends with `{` (possibly
            // with trailing whitespace).
            let mut check = line.end;
            while check > line.content_start
                && (bytes[check - 1] == b' ' || bytes[check - 1] == b'\t')
            {
                check -= 1;
            }
            if check > line.content_start && bytes[check - 1] == b'{' {
                // This is a braced freeform text block. Reclassify all
                // following lines as Value until we find a line that is
                // just `}` (possibly with surrounding whitespace).
                i += 1;
                while i < lines.len() {
                    let inner = &lines[i];
                    // Check if this line is just `}` (with optional whitespace).
                    let trimmed_start = inner.content_start;
                    let trimmed_end = inner.end;
                    if trimmed_start < trimmed_end
                        && bytes[trimmed_start] == b'}'
                        && is_only_closing_brace(bytes, trimmed_start, trimmed_end)
                    {
                        // The `}` line itself — reclassify as Value and stop.
                        lines[i].kind = LineKind::Value;
                        i += 1;
                        break;
                    }
                    // Reclassify as Value (unless it's a blank line, which we keep).
                    if inner.kind != LineKind::Blank {
                        lines[i].kind = LineKind::Value;
                    }
                    i += 1;
                }
                continue;
            }
        }
        i += 1;
    }
}

/// Check if from `start` to `end`, the content is just `}` optionally
/// followed by whitespace.
fn is_only_closing_brace(bytes: &[u8], start: usize, end: usize) -> bool {
    if start >= end || bytes[start] != b'}' {
        return false;
    }
    for &b in &bytes[start + 1..end] {
        if b != b' ' && b != b'\t' {
            return false;
        }
    }
    true
}

/// Classify a single line based on its content.
fn classify_line(
    source: &str,
    content_start: usize,
    content_end: usize,
    is_blank: bool,
) -> LineKind {
    if is_blank {
        return LineKind::Blank;
    }

    let bytes = source.as_bytes();

    // Comment?
    if content_start + 1 < content_end
        && bytes[content_start] == b'-'
        && bytes[content_start + 1] == b'-'
    {
        // Make sure it's `--` not `---foo` which could be a field name with
        // lots of hyphens. In practice `--` is always a comment start.
        return LineKind::Comment;
    }

    // Grab the first word.
    let (word_start, word_end) = scan_word(bytes, content_start);
    if word_start == word_end {
        // No word found — treat as value.
        return LineKind::Value;
    }
    let word = &source[word_start..word_end];

    // Section header?
    if is_section_keyword(word) {
        // A section keyword must be followed by EOL, whitespace + section arg,
        // or `{`. It must NOT be followed by punctuation like `.` — that
        // indicates a continuation/description line (e.g., "  library. The...").
        if word_end >= content_end {
            // Keyword at EOL (e.g., `library\n`).
            return LineKind::SectionHeader;
        }
        let ch = bytes[word_end];
        if ch == b' ' || ch == b'\t' || ch == b'{' {
            return LineKind::SectionHeader;
        }
    }

    // Conditional?
    if is_conditional_keyword(word) {
        let after_word = skip_hspace(bytes, word_end);
        if after_word >= content_end || bytes[after_word] != b':' {
            return LineKind::Conditional;
        }
    }

    // Field? Look for `:` after the first word (possibly with spaces).
    // Field names can contain letters, digits, hyphens; e.g. `build-depends:`
    let after_word = skip_hspace(bytes, word_end);
    if after_word < content_end && bytes[after_word] == b':' {
        return LineKind::Field;
    }

    // Otherwise, it's a value / continuation line.
    LineKind::Value
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Tokenize a `.cabal` source string into a flat list of [`Token`]s.
///
/// The returned token list always ends with a [`TokenKind::Eof`] token.
/// Every byte of the input is covered by either a token span or a trivia
/// piece on one of the tokens.
pub fn tokenize(source: &str) -> Vec<Token> {
    let lines = split_lines(source);
    let mut tokens = Vec::new();
    let mut pending_trivia: Vec<TriviaPiece> = Vec::new();

    for line in &lines {
        match line.kind {
            LineKind::Blank => {
                // The whole line (including newline) is trivia.
                if line.start < line.end {
                    pending_trivia.push(TriviaPiece {
                        kind: TriviaKind::Whitespace,
                        span: Span::new(line.start, line.end),
                    });
                }
                if line.newline_start < line.line_end_with_newline {
                    pending_trivia.push(TriviaPiece {
                        kind: TriviaKind::Newline,
                        span: Span::new(line.newline_start, line.line_end_with_newline),
                    });
                }
            }

            LineKind::Comment => {
                // Leading whitespace as trivia.
                if line.start < line.content_start {
                    pending_trivia.push(TriviaPiece {
                        kind: TriviaKind::Whitespace,
                        span: Span::new(line.start, line.content_start),
                    });
                }
                // The comment text itself.
                let comment_span = Span::new(line.content_start, line.end);
                // Emit comment as a standalone token so the parser can place
                // it in the CST. Attach accumulated trivia to it.
                let trivia = std::mem::take(&mut pending_trivia);
                // We don't add the comment to trivia — we emit it as a token.
                // But we need to handle the newline.
                tokens.push(Token {
                    kind: TokenKind::Comment,
                    span: comment_span,
                    indent: line.indent.unwrap_or(0),
                    leading_trivia: trivia,
                });
                // Newline after comment is trivia for the next token.
                if line.newline_start < line.line_end_with_newline {
                    pending_trivia.push(TriviaPiece {
                        kind: TriviaKind::Newline,
                        span: Span::new(line.newline_start, line.line_end_with_newline),
                    });
                }
            }

            LineKind::SectionHeader => {
                tokenize_section_header(source, line, &mut tokens, &mut pending_trivia);
            }

            LineKind::Conditional => {
                tokenize_conditional(source, line, &mut tokens, &mut pending_trivia);
            }

            LineKind::Field => {
                tokenize_field(source, line, &mut tokens, &mut pending_trivia);
            }

            LineKind::Value => {
                tokenize_value_line(source, line, &mut tokens, &mut pending_trivia);
            }
        }
    }

    // EOF token gets any remaining trivia.
    let eof_offset = source.len();
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::empty(eof_offset),
        indent: 0,
        leading_trivia: std::mem::take(&mut pending_trivia),
    });

    tokens
}

/// Tokenize a section header line like `executable my-exe` or `library`.
fn tokenize_section_header(
    source: &str,
    line: &RawLine,
    tokens: &mut Vec<Token>,
    pending_trivia: &mut Vec<TriviaPiece>,
) {
    let bytes = source.as_bytes();

    // Leading whitespace.
    if line.start < line.content_start {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(line.start, line.content_start),
        });
    }

    // The section keyword.
    let (kw_start, kw_end) = scan_word(bytes, line.content_start);
    tokens.push(Token {
        kind: TokenKind::SectionHeader,
        span: Span::new(kw_start, kw_end),
        indent: line.indent.unwrap_or(0),
        leading_trivia: std::mem::take(pending_trivia),
    });

    // After the keyword: optional whitespace + section argument(s).
    let mut pos = kw_end;
    // Whitespace between keyword and arg.
    let ws_start = pos;
    pos = skip_hspace(bytes, pos);
    if ws_start < pos {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(ws_start, pos),
        });
    }

    // Section argument: everything remaining on the line (trimmed).
    if pos < line.end {
        // Trim trailing whitespace from the arg.
        let mut arg_end = line.end;
        while arg_end > pos && (bytes[arg_end - 1] == b' ' || bytes[arg_end - 1] == b'\t') {
            arg_end -= 1;
        }
        if pos < arg_end {
            tokens.push(Token {
                kind: TokenKind::SectionArg,
                span: Span::new(pos, arg_end),
                indent: visual_column(bytes, line.start, pos),
                leading_trivia: std::mem::take(pending_trivia),
            });
            // Trailing whitespace as trivia.
            if arg_end < line.end {
                pending_trivia.push(TriviaPiece {
                    kind: TriviaKind::Whitespace,
                    span: Span::new(arg_end, line.end),
                });
            }
        }
    }

    // Newline.
    if line.newline_start < line.line_end_with_newline {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Newline,
            span: Span::new(line.newline_start, line.line_end_with_newline),
        });
    }
}

/// Tokenize a conditional line like `if flag(dev)` or `else`.
fn tokenize_conditional(
    source: &str,
    line: &RawLine,
    tokens: &mut Vec<Token>,
    pending_trivia: &mut Vec<TriviaPiece>,
) {
    let bytes = source.as_bytes();

    // Leading whitespace.
    if line.start < line.content_start {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(line.start, line.content_start),
        });
    }

    // The keyword (if / else / elif).
    let (kw_start, kw_end) = scan_word(bytes, line.content_start);
    let kw_str = &source[kw_start..kw_end];
    let kind = if kw_str.eq_ignore_ascii_case("if") {
        TokenKind::If
    } else if kw_str.eq_ignore_ascii_case("else") {
        TokenKind::Else
    } else {
        TokenKind::Elif
    };

    tokens.push(Token {
        kind,
        span: Span::new(kw_start, kw_end),
        indent: line.indent.unwrap_or(0),
        leading_trivia: std::mem::take(pending_trivia),
    });

    // For `if`/`elif`, tokenize the condition expression.
    // For `else`, check if there's remaining content (e.g. `else {`).
    if kind == TokenKind::If || kind == TokenKind::Elif {
        tokenize_condition_expr(source, bytes, kw_end, line, tokens, pending_trivia);
    } else if kind == TokenKind::Else {
        // Capture any remaining content after `else` (e.g. `{` for braced blocks).
        let after_kw = skip_hspace(bytes, kw_end);
        if after_kw < line.end {
            // There's content after `else` — emit whitespace + value.
            if kw_end < after_kw {
                pending_trivia.push(TriviaPiece {
                    kind: TriviaKind::Whitespace,
                    span: Span::new(kw_end, after_kw),
                });
            }
            tokens.push(Token {
                kind: TokenKind::Value,
                span: Span::new(after_kw, line.end),
                indent: visual_column(bytes, line.start, after_kw),
                leading_trivia: std::mem::take(pending_trivia),
            });
        }
    }

    // Newline.
    if line.newline_start < line.line_end_with_newline {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Newline,
            span: Span::new(line.newline_start, line.line_end_with_newline),
        });
    }
}

/// Tokenize the condition expression portion of an `if`/`elif` line.
///
/// E.g. for `if flag(dev) && !os(windows)`, this tokenizes everything
/// after `if`.
fn tokenize_condition_expr(
    _source: &str,
    bytes: &[u8],
    start: usize,
    line: &RawLine,
    tokens: &mut Vec<Token>,
    pending_trivia: &mut Vec<TriviaPiece>,
) {
    let end = line.end;
    let mut pos = start;

    while pos < end {
        let b = bytes[pos];
        match b {
            b' ' | b'\t' => {
                let ws_start = pos;
                pos = skip_hspace(bytes, pos);
                pending_trivia.push(TriviaPiece {
                    kind: TriviaKind::Whitespace,
                    span: Span::new(ws_start, pos),
                });
            }
            b'(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b'!' => {
                tokens.push(Token {
                    kind: TokenKind::Not,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b'&' => {
                if pos + 1 < end && bytes[pos + 1] == b'&' {
                    tokens.push(Token {
                        kind: TokenKind::And,
                        span: Span::new(pos, pos + 2),
                        indent: visual_column(bytes, line.start, pos),
                        leading_trivia: std::mem::take(pending_trivia),
                    });
                    pos += 2;
                } else {
                    // Stray `&` (not `&&`) — emit as single-char Value for error recovery.
                    tokens.push(Token {
                        kind: TokenKind::Value,
                        span: Span::new(pos, pos + 1),
                        indent: visual_column(bytes, line.start, pos),
                        leading_trivia: std::mem::take(pending_trivia),
                    });
                    pos += 1;
                }
            }
            b'|' => {
                if pos + 1 < end && bytes[pos + 1] == b'|' {
                    tokens.push(Token {
                        kind: TokenKind::Or,
                        span: Span::new(pos, pos + 2),
                        indent: visual_column(bytes, line.start, pos),
                        leading_trivia: std::mem::take(pending_trivia),
                    });
                    pos += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Value,
                        span: Span::new(pos, pos + 1),
                        indent: visual_column(bytes, line.start, pos),
                        leading_trivia: std::mem::take(pending_trivia),
                    });
                    pos += 1;
                }
            }
            b'>' if pos + 1 < end && bytes[pos + 1] == b'=' => {
                tokens.push(Token {
                    kind: TokenKind::CompOp,
                    span: Span::new(pos, pos + 2),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 2;
            }
            b'<' if pos + 1 < end && bytes[pos + 1] == b'=' => {
                tokens.push(Token {
                    kind: TokenKind::CompOp,
                    span: Span::new(pos, pos + 2),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 2;
            }
            b'=' => {
                let len = if pos + 1 < end && bytes[pos + 1] == b'=' {
                    2
                } else {
                    1
                };
                tokens.push(Token {
                    kind: TokenKind::CompOp,
                    span: Span::new(pos, pos + len),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += len;
            }
            b'>' => {
                tokens.push(Token {
                    kind: TokenKind::CompOp,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b'<' => {
                tokens.push(Token {
                    kind: TokenKind::CompOp,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    span: Span::new(pos, pos + 1),
                    indent: visual_column(bytes, line.start, pos),
                    leading_trivia: std::mem::take(pending_trivia),
                });
                pos += 1;
            }
            b'-' if pos + 1 < end && bytes[pos + 1] == b'-' => {
                // Inline comment — rest of line.
                pending_trivia.push(TriviaPiece {
                    kind: TriviaKind::Comment,
                    span: Span::new(pos, end),
                });
                pos = end;
            }
            _ => {
                // An identifier or version number — emit as Value. Always
                // consume at least one byte to guarantee forward progress
                // on inputs containing stray operator chars.
                let val_start = pos;
                pos += 1;
                while pos < end
                    && !matches!(
                        bytes[pos],
                        b' ' | b'\t' | b'(' | b')' | b'!' | b',' | b'&' | b'|' | b'>' | b'<' | b'='
                    )
                {
                    pos += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Value,
                    span: Span::new(val_start, pos),
                    indent: visual_column(bytes, line.start, val_start),
                    leading_trivia: std::mem::take(pending_trivia),
                });
            }
        }
    }
}

/// Tokenize a field line like `build-depends: base >=4.14`.
fn tokenize_field(
    source: &str,
    line: &RawLine,
    tokens: &mut Vec<Token>,
    pending_trivia: &mut Vec<TriviaPiece>,
) {
    let bytes = source.as_bytes();

    // Leading whitespace.
    if line.start < line.content_start {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(line.start, line.content_start),
        });
    }

    // Field name.
    let (name_start, name_end) = scan_word(bytes, line.content_start);
    tokens.push(Token {
        kind: TokenKind::FieldName,
        span: Span::new(name_start, name_end),
        indent: line.indent.unwrap_or(0),
        leading_trivia: std::mem::take(pending_trivia),
    });

    // Optional whitespace between name and colon.
    let mut pos = name_end;
    let ws_start = pos;
    pos = skip_hspace(bytes, pos);
    if ws_start < pos {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(ws_start, pos),
        });
    }

    // Colon.
    if pos < line.end && bytes[pos] == b':' {
        tokens.push(Token {
            kind: TokenKind::Colon,
            span: Span::new(pos, pos + 1),
            indent: visual_column(bytes, line.start, pos),
            leading_trivia: std::mem::take(pending_trivia),
        });
        pos += 1;
    }

    // Optional whitespace after colon.
    let ws_start2 = pos;
    pos = skip_hspace(bytes, pos);
    if ws_start2 < pos {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(ws_start2, pos),
        });
    }

    // Rest of line is the value (if non-empty).
    if pos < line.end {
        // Check for inline comment at the end.
        let val_end = line.end;
        tokens.push(Token {
            kind: TokenKind::Value,
            span: Span::new(pos, val_end),
            indent: visual_column(bytes, line.start, pos),
            leading_trivia: std::mem::take(pending_trivia),
        });
    }

    // Newline.
    if line.newline_start < line.line_end_with_newline {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Newline,
            span: Span::new(line.newline_start, line.line_end_with_newline),
        });
    }
}

/// Tokenize a continuation / value line (no field name, no section header).
fn tokenize_value_line(
    source: &str,
    line: &RawLine,
    tokens: &mut Vec<Token>,
    pending_trivia: &mut Vec<TriviaPiece>,
) {
    let _ = source;

    // Leading whitespace.
    if line.start < line.content_start {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Whitespace,
            span: Span::new(line.start, line.content_start),
        });
    }

    if line.content_start < line.end {
        tokens.push(Token {
            kind: TokenKind::Value,
            span: Span::new(line.content_start, line.end),
            indent: line.indent.unwrap_or(0),
            leading_trivia: std::mem::take(pending_trivia),
        });
    }

    // Newline.
    if line.newline_start < line.line_end_with_newline {
        pending_trivia.push(TriviaPiece {
            kind: TriviaKind::Newline,
            span: Span::new(line.newline_start, line.line_end_with_newline),
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: collect just the (kind, text) pairs from tokenization.
    fn tok_pairs(source: &str) -> Vec<(TokenKind, &str)> {
        let tokens = tokenize(source);
        tokens
            .iter()
            .map(|t| (t.kind, t.span.slice(source)))
            .collect()
    }

    #[test]
    fn lex_simple_field() {
        let src = "name: foo\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "name"),
                (TokenKind::Colon, ":"),
                (TokenKind::Value, "foo"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_field_with_spaces() {
        let src = "build-depends:    base >=4.14\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "build-depends"),
                (TokenKind::Colon, ":"),
                (TokenKind::Value, "base >=4.14"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_section_header_no_arg() {
        let src = "library\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![(TokenKind::SectionHeader, "library"), (TokenKind::Eof, ""),]
        );
    }

    #[test]
    fn lex_section_header_with_arg() {
        let src = "executable my-exe\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::SectionHeader, "executable"),
                (TokenKind::SectionArg, "my-exe"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_conditional_if() {
        let src = "  if flag(dev)\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::If, "if"),
                (TokenKind::Value, "flag"),
                (TokenKind::LParen, "("),
                (TokenKind::Value, "dev"),
                (TokenKind::RParen, ")"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_conditional_complex() {
        let src = "  if flag(dev) && !os(windows)\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::If, "if"),
                (TokenKind::Value, "flag"),
                (TokenKind::LParen, "("),
                (TokenKind::Value, "dev"),
                (TokenKind::RParen, ")"),
                (TokenKind::And, "&&"),
                (TokenKind::Not, "!"),
                (TokenKind::Value, "os"),
                (TokenKind::LParen, "("),
                (TokenKind::Value, "windows"),
                (TokenKind::RParen, ")"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_else() {
        let src = "  else\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![(TokenKind::Else, "else"), (TokenKind::Eof, ""),]
        );
    }

    #[test]
    fn lex_comment_line() {
        let src = "-- this is a comment\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::Comment, "-- this is a comment"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_blank_lines() {
        let src = "name: foo\n\nversion: 0.1\n";
        let tokens = tokenize(src);
        // The blank line should be trivia on the `version` field name token.
        let version_tok = tokens
            .iter()
            .find(|t| t.kind == TokenKind::FieldName && t.span.slice(src) == "version");
        assert!(version_tok.is_some());
        let trivia_kinds: Vec<_> = version_tok
            .unwrap()
            .leading_trivia
            .iter()
            .map(|t| t.kind)
            .collect();
        // Should include newline(s) from the blank line.
        assert!(trivia_kinds.contains(&TriviaKind::Newline));
    }

    #[test]
    fn lex_indented_field() {
        let src = "  exposed-modules: Foo\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "exposed-modules"),
                (TokenKind::Colon, ":"),
                (TokenKind::Value, "Foo"),
                (TokenKind::Eof, ""),
            ]
        );
        // Check indent.
        let tokens = tokenize(src);
        assert_eq!(tokens[0].indent, 2);
    }

    #[test]
    fn lex_continuation_value() {
        let src = "    base >=4.14\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![(TokenKind::Value, "base >=4.14"), (TokenKind::Eof, ""),]
        );
        let tokens = tokenize(src);
        assert_eq!(tokens[0].indent, 4);
    }

    #[test]
    fn lex_full_span_coverage() {
        let src = "name: foo\nversion: 0.1\n";
        let tokens = tokenize(src);
        // Collect all byte offsets covered.
        let mut covered = vec![false; src.len()];
        for tok in &tokens {
            for tp in &tok.leading_trivia {
                for i in tp.span.start..tp.span.end {
                    assert!(
                        !covered[i],
                        "byte {i} covered twice (trivia on {:?})",
                        tok.kind
                    );
                    covered[i] = true;
                }
            }
            for i in tok.span.start..tok.span.end {
                assert!(!covered[i], "byte {i} covered twice (token {:?})", tok.kind);
                covered[i] = true;
            }
        }
        for (i, &c) in covered.iter().enumerate() {
            assert!(c, "byte {i} ({:?}) not covered", src.as_bytes()[i] as char);
        }
    }

    #[test]
    fn lex_impl_condition() {
        let src = "  if impl(ghc >= 9.6)\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::If, "if"),
                (TokenKind::Value, "impl"),
                (TokenKind::LParen, "("),
                (TokenKind::Value, "ghc"),
                (TokenKind::CompOp, ">="),
                (TokenKind::Value, "9.6"),
                (TokenKind::RParen, ")"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_field_no_value() {
        let src = "build-depends:\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "build-depends"),
                (TokenKind::Colon, ":"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_import_as_field() {
        // `import:` should lex as a regular field name.
        let src = "  import: warnings\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "import"),
                (TokenKind::Colon, ":"),
                (TokenKind::Value, "warnings"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_tab_indent() {
        let src = "\texposed-modules: Foo\n";
        let tokens = tokenize(src);
        // Tab should expand to column 8.
        assert_eq!(tokens[0].indent, 8);
    }

    #[test]
    fn lex_no_trailing_newline() {
        let src = "name: foo";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::FieldName, "name"),
                (TokenKind::Colon, ":"),
                (TokenKind::Value, "foo"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn lex_common_stanza() {
        let src = "common warnings\n";
        let pairs = tok_pairs(src);
        assert_eq!(
            pairs,
            vec![
                (TokenKind::SectionHeader, "common"),
                (TokenKind::SectionArg, "warnings"),
                (TokenKind::Eof, ""),
            ]
        );
    }

    #[test]
    fn full_span_coverage_multiline() {
        let src = "cabal-version: 3.0\nname: foo\n\n-- A comment\n\nlibrary\n  exposed-modules: Foo\n  build-depends:\n    base >=4.14\n";
        let tokens = tokenize(src);
        let mut covered = vec![false; src.len()];
        for tok in &tokens {
            for tp in &tok.leading_trivia {
                for i in tp.span.start..tp.span.end {
                    assert!(!covered[i], "byte {i} covered twice (trivia)");
                    covered[i] = true;
                }
            }
            for i in tok.span.start..tok.span.end {
                assert!(!covered[i], "byte {i} covered twice (token {:?})", tok.kind);
                covered[i] = true;
            }
        }
        for (i, &c) in covered.iter().enumerate() {
            assert!(c, "byte {i} ({:?}) not covered", src.as_bytes()[i] as char);
        }
    }
}
