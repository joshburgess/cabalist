//! Concrete Syntax Tree (CST) for `.cabal` files.
//!
//! The CST is a flat arena of nodes that mirrors the exact structure of the
//! `.cabal` file, preserving all formatting details — whitespace, comments,
//! blank lines, indentation style. The `render()` method reproduces the
//! original source byte-for-byte when no edits have been made.

use crate::lexer::TriviaPiece;
use crate::span::{NodeId, Span};

/// The kind of a CST node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CstNodeKind {
    /// The root container. Its children are top-level fields, sections,
    /// comments, and blank lines.
    Root,
    /// A field: `field-name: value` (possibly with multi-line continuation).
    Field,
    /// A section: `library`, `executable foo`, `common warnings`, etc.
    /// Children are the section body (fields, conditionals, imports, comments).
    Section,
    /// A conditional block: `if condition` + body, with optional `else` block.
    Conditional,
    /// An `import: stanza-name` directive inside a section.
    Import,
    /// A standalone comment line.
    Comment,
    /// A preserved blank line between stanzas or fields.
    BlankLine,
    /// A value continuation line that is a direct child of its parent field
    /// or section. Used to preserve multi-line field values.
    ValueLine,
    /// An `else` block attached to a `Conditional` node.
    ElseBlock,
}

/// A single node in the CST arena.
#[derive(Debug, Clone)]
pub struct CstNode {
    /// What kind of syntax element this node represents.
    pub kind: CstNodeKind,

    /// Full byte span of this node in the source, including leading trivia.
    pub span: Span,

    /// Span of just the meaningful content (excluding leading/trailing trivia
    /// that belongs to this node).
    pub content_span: Span,

    /// Children of this node (indices into the arena).
    pub children: Vec<NodeId>,

    /// Parent node (None only for the Root).
    pub parent: Option<NodeId>,

    // -- Field-specific spans --
    /// For `Field` / `Import` nodes: span of the field name.
    pub field_name: Option<Span>,

    /// For `Field` / `Import` nodes: span of the field value (first line
    /// only; continuation lines are child `ValueLine` nodes).
    pub field_value: Option<Span>,

    // -- Section-specific spans --
    /// For `Section` nodes: span of the section keyword.
    pub section_keyword: Option<Span>,

    /// For `Section` nodes: span of the section argument (e.g. `my-exe`).
    pub section_arg: Option<Span>,

    // -- Conditional-specific spans --
    /// For `Conditional` nodes: span of the keyword (`if` / `elif`).
    pub condition_keyword: Option<Span>,

    /// For `Conditional` nodes: span of the condition expression text.
    pub condition_expr: Option<Span>,

    /// Leading trivia pieces (whitespace, newlines, comments) that precede
    /// this node's content.
    pub leading_trivia: Vec<TriviaPiece>,

    /// Trailing trivia pieces (typically a newline at the end of the line).
    pub trailing_trivia: Vec<TriviaPiece>,

    /// The indentation level (visual column) of this node.
    pub indent: usize,
}

impl CstNode {
    /// Create a new node with the given kind, defaulting all optional fields
    /// to `None` / empty.
    pub fn new(kind: CstNodeKind, span: Span) -> Self {
        Self {
            kind,
            span,
            content_span: span,
            children: Vec::new(),
            parent: None,
            field_name: None,
            field_value: None,
            section_keyword: None,
            section_arg: None,
            condition_keyword: None,
            condition_expr: None,
            leading_trivia: Vec::new(),
            trailing_trivia: Vec::new(),
            indent: 0,
        }
    }
}

/// The concrete syntax tree for a `.cabal` file.
///
/// All [`Span`]s reference byte offsets into [`source`](CabalCst::source).
#[derive(Debug, Clone)]
pub struct CabalCst {
    /// The original source text (owned).
    pub source: String,

    /// Flat arena of all CST nodes.
    pub nodes: Vec<CstNode>,

    /// Index of the root node (always `NodeId(0)`).
    pub root: NodeId,
}

impl CabalCst {
    /// Render the CST back to text. When no edits have been made, this must
    /// produce byte-identical output to the original source.
    pub fn render(&self) -> String {
        // Strategy: walk every node in the tree in source order and emit
        // their trivia + content spans. Since the arena stores nodes in
        // source order (they are added during a left-to-right parse), we
        // can do a depth-first traversal from the root and collect spans.
        //
        // However, the simplest correct approach for an un-edited CST is
        // to just return the source. For an edited CST we need the full
        // render. We implement the full render so it works in both cases.
        let mut out = String::with_capacity(self.source.len());
        self.render_node(self.root, &mut out);
        out
    }

    /// Recursively render a single node and its descendants.
    fn render_node(&self, node_id: NodeId, out: &mut String) {
        let node = &self.nodes[node_id.0];

        // Emit leading trivia.
        for tp in &node.leading_trivia {
            out.push_str(tp.span.slice(&self.source));
        }

        // Emit the node's own content based on kind.
        match node.kind {
            CstNodeKind::Root => {
                // Root has no content of its own; just render children.
                for &child_id in &node.children {
                    self.render_node(child_id, out);
                }
            }

            CstNodeKind::Field => {
                // field name
                if let Some(ref name_span) = node.field_name {
                    out.push_str(name_span.slice(&self.source));
                }
                // The colon and spacing between name, colon, and value are
                // captured in the content_span. We emit the content_span
                // region that isn't the field_name or field_value.
                //
                // Actually, we store the full line content between
                // field_name.end and field_value.start (colon + spacing) as
                // part of content_span. Let's emit the "middle" region.
                let name_end = node
                    .field_name
                    .map(|s| s.end)
                    .unwrap_or(node.content_span.start);
                let value_start = node
                    .field_value
                    .map(|s| s.start)
                    .unwrap_or(node.content_span.end);
                // Middle: everything between field name and value.
                if name_end < value_start {
                    out.push_str(&self.source[name_end..value_start]);
                }
                // field value (first line)
                if let Some(ref val_span) = node.field_value {
                    out.push_str(val_span.slice(&self.source));
                }
                // Trailing trivia (newline).
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
                // Children: continuation ValueLine nodes.
                for &child_id in &node.children {
                    self.render_node(child_id, out);
                }
            }

            CstNodeKind::Import => {
                // Same structure as Field.
                if let Some(ref name_span) = node.field_name {
                    out.push_str(name_span.slice(&self.source));
                }
                let name_end = node
                    .field_name
                    .map(|s| s.end)
                    .unwrap_or(node.content_span.start);
                let value_start = node
                    .field_value
                    .map(|s| s.start)
                    .unwrap_or(node.content_span.end);
                if name_end < value_start {
                    out.push_str(&self.source[name_end..value_start]);
                }
                if let Some(ref val_span) = node.field_value {
                    out.push_str(val_span.slice(&self.source));
                }
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
            }

            CstNodeKind::Section => {
                // Section keyword.
                if let Some(ref kw_span) = node.section_keyword {
                    out.push_str(kw_span.slice(&self.source));
                }
                // Spacing + arg.
                let kw_end = node
                    .section_keyword
                    .map(|s| s.end)
                    .unwrap_or(node.content_span.start);
                let arg_start = node.section_arg.map(|s| s.start);
                let arg_end = node.section_arg.map(|s| s.end);
                match (arg_start, arg_end) {
                    (Some(astart), Some(aend)) => {
                        // Spacing between keyword and arg.
                        if kw_end < astart {
                            out.push_str(&self.source[kw_end..astart]);
                        }
                        out.push_str(&self.source[astart..aend]);
                        // Anything between arg end and content_span end
                        // (trailing whitespace on the header line).
                        if aend < node.content_span.end {
                            out.push_str(&self.source[aend..node.content_span.end]);
                        }
                    }
                    _ => {
                        // No arg — emit any trailing content.
                        if kw_end < node.content_span.end {
                            out.push_str(&self.source[kw_end..node.content_span.end]);
                        }
                    }
                }
                // Trailing trivia (newline after header).
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
                // Section body children.
                for &child_id in &node.children {
                    self.render_node(child_id, out);
                }
            }

            CstNodeKind::Conditional => {
                // Keyword (if / elif).
                if let Some(ref kw_span) = node.condition_keyword {
                    out.push_str(kw_span.slice(&self.source));
                }
                let kw_end = node
                    .condition_keyword
                    .map(|s| s.end)
                    .unwrap_or(node.content_span.start);
                // Condition expression.
                if let Some(ref expr_span) = node.condition_expr {
                    if kw_end < expr_span.start {
                        out.push_str(&self.source[kw_end..expr_span.start]);
                    }
                    out.push_str(expr_span.slice(&self.source));
                    if expr_span.end < node.content_span.end {
                        out.push_str(&self.source[expr_span.end..node.content_span.end]);
                    }
                } else if kw_end < node.content_span.end {
                    out.push_str(&self.source[kw_end..node.content_span.end]);
                }
                // Trailing trivia.
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
                // Children (then-block fields + optional ElseBlock).
                for &child_id in &node.children {
                    self.render_node(child_id, out);
                }
            }

            CstNodeKind::ElseBlock => {
                // The `else` keyword line.
                out.push_str(node.content_span.slice(&self.source));
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
                // Else body children.
                for &child_id in &node.children {
                    self.render_node(child_id, out);
                }
            }

            CstNodeKind::Comment => {
                out.push_str(node.content_span.slice(&self.source));
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
            }

            CstNodeKind::BlankLine => {
                out.push_str(node.content_span.slice(&self.source));
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
            }

            CstNodeKind::ValueLine => {
                out.push_str(node.content_span.slice(&self.source));
                for tp in &node.trailing_trivia {
                    out.push_str(tp.span.slice(&self.source));
                }
            }
        }
    }

    /// Number of nodes in the arena.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get a reference to a node by its id.
    pub fn node(&self, id: NodeId) -> &CstNode {
        &self.nodes[id.0]
    }

    /// Get a mutable reference to a node by its id.
    pub fn node_mut(&mut self, id: NodeId) -> &mut CstNode {
        &mut self.nodes[id.0]
    }

    /// Iterate over the direct children of a node.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.0].children
    }
}
