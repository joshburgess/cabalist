//! Span (byte offset) <-> LSP Position (line:col UTF-16) conversion.
//!
//! The parser uses `Span { start, end }` as byte offsets into the source.
//! LSP uses `Position { line, character }` where `character` is a UTF-16
//! code unit offset. This module bridges the two.

use cabalist_parser::span::Span;
use tower_lsp::lsp_types::{Position, Range};

/// Pre-computed line start offsets for fast byte-offset <-> Position conversion.
pub struct LineIndex {
    /// Byte offset of the start of each line. `line_starts[0]` is always `0`.
    line_starts: Vec<usize>,
    /// The source text (needed for UTF-16 offset computation).
    source: String,
}

impl LineIndex {
    /// Build a line index from source text.
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            line_starts,
            source: source.to_string(),
        }
    }

    /// Convert a byte offset to an LSP `Position`.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let offset = offset.min(self.source.len());
        let line = self.line_starts.partition_point(|&start| start <= offset).saturating_sub(1);
        let line_start = self.line_starts[line];
        // Count UTF-16 code units from line start to offset.
        let col_utf16: u32 = self.source[line_start..offset]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();
        Position {
            line: line as u32,
            character: col_utf16,
        }
    }

    /// Convert an LSP `Position` to a byte offset.
    pub fn position_to_offset(&self, pos: Position) -> usize {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return self.source.len();
        }
        let line_start = self.line_starts[line];
        let mut utf16_count = 0u32;
        let mut byte_offset = line_start;
        for c in self.source[line_start..].chars() {
            if utf16_count >= pos.character {
                break;
            }
            if c == '\n' {
                break;
            }
            utf16_count += c.len_utf16() as u32;
            byte_offset += c.len_utf8();
        }
        byte_offset
    }

    /// Convert a parser `Span` to an LSP `Range`.
    pub fn span_to_range(&self, span: Span) -> Range {
        Range {
            start: self.offset_to_position(span.start),
            end: self.offset_to_position(span.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_ascii() {
        let source = "hello\nworld\n";
        let idx = LineIndex::new(source);

        assert_eq!(idx.offset_to_position(0), Position { line: 0, character: 0 });
        assert_eq!(idx.offset_to_position(5), Position { line: 0, character: 5 });
        assert_eq!(idx.offset_to_position(6), Position { line: 1, character: 0 });
        assert_eq!(idx.offset_to_position(8), Position { line: 1, character: 2 });
    }

    #[test]
    fn roundtrip() {
        let source = "name: foo\nversion: 0.1\nlibrary\n  build-depends: base\n";
        let idx = LineIndex::new(source);

        for offset in 0..source.len() {
            if source.is_char_boundary(offset) {
                let pos = idx.offset_to_position(offset);
                let back = idx.position_to_offset(pos);
                assert_eq!(back, offset, "roundtrip failed at offset {offset}");
            }
        }
    }

    #[test]
    fn empty_source() {
        let idx = LineIndex::new("");
        assert_eq!(idx.offset_to_position(0), Position { line: 0, character: 0 });
    }

    #[test]
    fn span_to_range_basic() {
        let source = "name: foo\nversion: 0.1\n";
        let idx = LineIndex::new(source);
        let range = idx.span_to_range(Span::new(6, 9)); // "foo"
        assert_eq!(range.start, Position { line: 0, character: 6 });
        assert_eq!(range.end, Position { line: 0, character: 9 });
    }

    #[test]
    fn span_across_lines() {
        let source = "name: foo\nversion: 0.1\n";
        let idx = LineIndex::new(source);
        let range = idx.span_to_range(Span::new(0, 22)); // whole file
        assert_eq!(range.start, Position { line: 0, character: 0 });
        assert_eq!(range.end, Position { line: 1, character: 12 });
    }

    #[test]
    fn offset_past_end() {
        let source = "hi\n";
        let idx = LineIndex::new(source);
        let pos = idx.offset_to_position(100);
        assert_eq!(pos, Position { line: 1, character: 0 });
    }
}
