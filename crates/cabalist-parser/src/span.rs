//! Byte-offset spans and node identifiers for the CST arena.

/// A byte-offset range into the source text. Both `start` and `end` are
/// byte indices; the range is half-open: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Create a new span covering `[start, end)`.
    #[inline]
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "Span start ({start}) > end ({end})");
        Self { start, end }
    }

    /// A zero-length span at the given offset.
    #[inline]
    pub fn empty(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    /// Length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the span covers zero bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether this span fully contains `other`.
    #[inline]
    pub fn contains(&self, other: &Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Return the smallest span that covers both `self` and `other`.
    #[inline]
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Slice the referenced source text.
    #[inline]
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// An index into the CST node arena (`CabalCst::nodes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_basics() {
        let s = Span::new(2, 7);
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());

        let e = Span::empty(3);
        assert_eq!(e.len(), 0);
        assert!(e.is_empty());
    }

    #[test]
    fn span_contains() {
        let outer = Span::new(0, 10);
        let inner = Span::new(2, 5);
        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
    }

    #[test]
    fn span_merge() {
        let a = Span::new(0, 5);
        let b = Span::new(3, 10);
        let m = a.merge(&b);
        assert_eq!(m, Span::new(0, 10));
    }

    #[test]
    fn span_slice() {
        let src = "hello world";
        let s = Span::new(6, 11);
        assert_eq!(s.slice(src), "world");
    }
}
