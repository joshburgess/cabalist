//! Per-document state management.

use crate::convert::LineIndex;

/// State tracked for each open `.cabal` document.
pub struct DocumentState {
    /// The current full source text.
    pub source: String,
    /// Pre-computed line index for offset <-> position conversion.
    pub line_index: LineIndex,
    /// LSP document version (for staleness detection).
    pub version: i32,
}

impl DocumentState {
    /// Create a new document state from source text and version.
    pub fn new(source: String, version: i32) -> Self {
        let line_index = LineIndex::new(&source);
        Self {
            source,
            line_index,
            version,
        }
    }

    /// Update the document with new source text and version.
    pub fn update(&mut self, source: String, version: i32) {
        self.line_index = LineIndex::new(&source);
        self.source = source;
        self.version = version;
    }
}
