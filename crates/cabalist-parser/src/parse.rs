//! Hand-written recursive descent parser for `.cabal` files.
//!
//! Transforms the token stream from the lexer into a CST. The parser uses
//! indentation levels to determine nesting — sections contain fields that
//! are indented more than the section header, fields contain continuation
//! lines indented more than the field name, etc.

use crate::cst::{CabalCst, CstNode, CstNodeKind};
use crate::diagnostic::Diagnostic;
use crate::lexer::{tokenize, Token, TokenKind, TriviaKind};
use crate::span::{NodeId, Span};

/// The result of parsing a `.cabal` file.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The concrete syntax tree.
    pub cst: CabalCst,
    /// Diagnostics (errors, warnings) encountered during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parser state.
struct Parser {
    /// The original source text.
    source: String,
    /// The token stream (from the lexer).
    tokens: Vec<Token>,
    /// Current position in the token stream.
    pos: usize,
    /// The CST node arena being built.
    nodes: Vec<CstNode>,
    /// Diagnostics collected during parsing.
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(source: String, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            nodes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    // -- Token access -------------------------------------------------------

    /// Peek at the current token without consuming it.
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    /// Check if we're at EOF.
    fn at_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    // -- Node creation ------------------------------------------------------

    /// Allocate a new node in the arena, returning its `NodeId`.
    fn alloc_node(&mut self, node: CstNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Set the parent of `child` to `parent` and add `child` to `parent`'s
    /// children list.
    fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child.0].parent = Some(parent);
        self.nodes[parent.0].children.push(child);
    }

    // -- Diagnostic helpers -------------------------------------------------

    fn emit_error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }

    // -- Skip / advance helpers ---------------------------------------------

    // -- Parsing entry point ------------------------------------------------

    /// Parse the entire `.cabal` file, producing a `ParseResult`.
    fn parse(mut self) -> ParseResult {
        // Create the root node.
        let root_node = CstNode::new(CstNodeKind::Root, Span::new(0, self.source.len()));
        let root_id = self.alloc_node(root_node);

        // Parse top-level items.
        self.parse_body(root_id, 0, true);

        // Absorb any trailing trivia from the Eof token into the root.
        if !self.at_eof() {
            // Shouldn't happen, but be defensive.
        }

        // Update the root span.
        self.nodes[root_id.0].span = Span::new(0, self.source.len());
        self.nodes[root_id.0].content_span = Span::new(0, self.source.len());

        ParseResult {
            cst: CabalCst {
                source: self.source,
                nodes: self.nodes,
                root: root_id,
            },
            diagnostics: self.diagnostics,
        }
    }

    /// Parse a body (sequence of fields, sections, conditionals, comments,
    /// blank lines) where all items have indent > `min_indent`.
    ///
    /// For the top-level, `min_indent` is 0, `is_root` is true, and we
    /// accept items at indent 0.
    /// For section bodies, `min_indent` is the section header's indent,
    /// `is_root` is false, and we accept only items with indent > `min_indent`.
    fn parse_body(&mut self, parent_id: NodeId, min_indent: usize, is_root: bool) {
        loop {
            if self.at_eof() {
                // Absorb EOF trivia into the parent — but only once (at
                // the root level) to avoid duplicating trailing content.
                if is_root {
                    let eof_tok = self.peek();
                    if !eof_tok.leading_trivia.is_empty() {
                        self.consume_trailing_trivia(parent_id);
                    }
                }
                break;
            }

            let tok = self.peek();
            let indent = tok.indent;

            // For section bodies (is_root == false): items must be indented
            // more than the parent. For the top-level root: accept all.
            if !is_root && indent <= min_indent {
                break;
            }

            match tok.kind {
                TokenKind::Comment => {
                    let node_id = self.parse_comment();
                    self.add_child(parent_id, node_id);
                }
                TokenKind::SectionHeader => {
                    let node_id = self.parse_section();
                    self.add_child(parent_id, node_id);
                }
                TokenKind::If | TokenKind::Elif => {
                    let node_id = self.parse_conditional(indent);
                    self.add_child(parent_id, node_id);
                }
                TokenKind::Else => {
                    // `else` not expected here — it's handled inside
                    // parse_conditional. If we see it standalone, that's
                    // an error. Break so the caller can handle it.
                    break;
                }
                TokenKind::FieldName => {
                    // Check if this is an `import:` field.
                    let is_import = {
                        let name_text = tok.span.slice(&self.source);
                        name_text.eq_ignore_ascii_case("import")
                    };
                    if is_import {
                        let node_id = self.parse_import();
                        self.add_child(parent_id, node_id);
                    } else {
                        let node_id = self.parse_field(indent);
                        self.add_child(parent_id, node_id);
                    }
                }
                TokenKind::Value => {
                    // A value line at the body level — this could be a
                    // continuation of the previous field, but since we handle
                    // continuations inside parse_field, seeing one here means
                    // it's either misindented or a free-standing value.
                    if is_root && indent == 0 {
                        // Top-level value line — unusual but we should handle
                        // it. Emit as an error + skip.
                        let span = tok.span;
                        self.emit_error(span, "unexpected value at top level");
                        self.pos += 1;
                    } else if indent > min_indent {
                        // Indented value line in a section body — could be
                        // a continuation or orphan. Emit as a ValueLine child.
                        let node_id = self.parse_value_line();
                        self.add_child(parent_id, node_id);
                    } else {
                        break;
                    }
                }
                TokenKind::Eof => break,
                _ => {
                    // Unexpected token — skip with diagnostic.
                    let span = tok.span;
                    let kind = tok.kind;
                    self.emit_error(span, format!("unexpected token: {kind:?}"));
                    self.pos += 1;
                }
            }
        }
    }

    /// Consume remaining trivia from the EOF token and create blank/comment
    /// nodes as needed so they appear in the rendered output.
    fn consume_trailing_trivia(&mut self, parent_id: NodeId) {
        let eof_idx = self.pos.min(self.tokens.len() - 1);
        if self.tokens[eof_idx].kind != TokenKind::Eof {
            return;
        }

        // Take (not clone) the trivia so it can't be consumed again.
        let trivia: Vec<_> = std::mem::take(&mut self.tokens[eof_idx].leading_trivia);
        if trivia.is_empty() {
            return;
        }

        // Gather all the trivia into a single BlankLine node that covers
        // the full range. This is a simplification — it means trailing
        // blank lines at the end of the file get rendered correctly.
        let start = trivia.first().unwrap().span.start;
        let end = trivia.last().unwrap().span.end;
        let span = Span::new(start, end);

        let mut node = CstNode::new(CstNodeKind::BlankLine, span);
        node.content_span = span;
        // We store the trivia so render works: the BlankLine node's
        // content_span covers the text directly.
        let node_id = self.alloc_node(node);
        self.add_child(parent_id, node_id);
    }

    // -- Individual node parsers --------------------------------------------

    /// Parse a standalone comment line.
    fn parse_comment(&mut self) -> NodeId {
        let tok = self.peek().clone();
        debug_assert_eq!(tok.kind, TokenKind::Comment);
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::Comment, tok.span);
        node.leading_trivia = tok.leading_trivia;
        node.content_span = tok.span;
        node.indent = tok.indent;

        // Grab the newline trivia that follows.
        self.collect_trailing_newline(&mut node);

        // Update span to cover leading trivia + content + trailing trivia.
        self.finalize_node_span(&mut node);

        self.alloc_node(node)
    }

    /// Parse a field: `field-name: value` with possible continuation lines.
    fn parse_field(&mut self, field_indent: usize) -> NodeId {
        // We expect: FieldName, Colon, optional Value.
        let name_tok = self.peek().clone();
        debug_assert_eq!(name_tok.kind, TokenKind::FieldName);
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::Field, name_tok.span);
        node.leading_trivia = name_tok.leading_trivia;
        node.field_name = Some(name_tok.span);
        node.indent = name_tok.indent;

        // Expect Colon.
        let mut colon_end = name_tok.span.end;
        if !self.at_eof() && self.peek().kind == TokenKind::Colon {
            let colon_tok = self.peek().clone();
            // Absorb colon trivia (spacing between name and colon).
            colon_end = colon_tok.span.end;
            self.pos += 1;

            // Check for value on the same line.
            if !self.at_eof() && self.peek().kind == TokenKind::Value {
                let val_tok = self.peek();
                // Only take this value if it's not on a new line.
                // We detect "same line" by checking that the value token's
                // leading trivia does NOT contain a Newline.
                let has_newline = val_tok
                    .leading_trivia
                    .iter()
                    .any(|t| t.kind == TriviaKind::Newline);
                if !has_newline {
                    let val_tok = self.peek().clone();
                    node.field_value = Some(val_tok.span);
                    colon_end = val_tok.span.end;
                    self.pos += 1;
                }
            }
        }

        node.content_span = Span::new(name_tok.span.start, colon_end);

        // Check if the field value opens a braced freeform text block.
        // E.g. `Description: {` — the value text ends with `{`.
        let is_braced_field = node.field_value.is_some_and(|val_span| {
            let val_text = val_span.slice(&self.source);
            val_text.trim_end().ends_with('{')
        });

        // Collect trailing newline.
        self.collect_trailing_newline(&mut node);

        if is_braced_field {
            // Braced freeform text block: consume all lines until `}`
            // regardless of indentation.
            self.parse_braced_field_continuation(&mut node);
        } else {
            // Continuation lines: any following line with indent > field_indent.
            self.parse_continuation_lines(&mut node, field_indent);
        }

        self.finalize_node_span(&mut node);
        self.alloc_node(node)
    }

    /// Parse continuation lines for a braced freeform text field (`Description: { ... }`).
    /// Consumes all lines until a line whose content is just `}`.
    fn parse_braced_field_continuation(&mut self, field_node: &mut CstNode) {
        let mut child_ids = Vec::new();

        loop {
            if self.at_eof() {
                break;
            }
            let tok = self.peek();

            // Check if this token's text is just `}` (the closing brace line).
            let is_closing =
                tok.kind == TokenKind::Value && tok.span.slice(&self.source).trim() == "}";

            match tok.kind {
                TokenKind::Value | TokenKind::Comment => {
                    let node_id = if tok.kind == TokenKind::Comment {
                        self.parse_comment()
                    } else {
                        self.parse_value_line()
                    };
                    child_ids.push(node_id);
                    if is_closing {
                        break;
                    }
                }
                _ => {
                    // In braced mode, other token types (blank lines represented
                    // via trivia) are handled by the value/comment lines above.
                    // If we hit something unexpected, just break.
                    break;
                }
            }
        }

        field_node.children = child_ids;
    }

    /// Parse continuation lines for a field. These are lines indented more
    /// than the field's indent level.
    fn parse_continuation_lines(&mut self, field_node: &mut CstNode, field_indent: usize) {
        // Collect continuation lines as standalone nodes — their IDs will
        // be stored in the field node's children vec.
        let mut child_ids = Vec::new();

        loop {
            if self.at_eof() {
                break;
            }
            let tok = self.peek();
            let indent = tok.indent;

            // A continuation line must be indented more than the field.
            if indent <= field_indent {
                // But check if this is a blank line or comment that might be
                // "between" continuation lines.
                if tok.kind == TokenKind::Comment {
                    // Check if the next non-comment/non-blank token is still
                    // a continuation. For now, treat indented comments as
                    // part of the field too.
                    if indent > field_indent {
                        let node_id = self.parse_comment();
                        child_ids.push(node_id);
                        continue;
                    }
                }
                break;
            }

            match tok.kind {
                TokenKind::Value => {
                    let node_id = self.parse_value_line();
                    child_ids.push(node_id);
                }
                TokenKind::FieldName => {
                    // If a field name appears indented deeper, it might be a
                    // nested field (unlikely in .cabal) or misformatted.
                    // Treat the entire line as a value for now.
                    let node_id = self.parse_value_line_from_field();
                    child_ids.push(node_id);
                }
                TokenKind::Comment => {
                    let node_id = self.parse_comment();
                    child_ids.push(node_id);
                }
                _ => {
                    break;
                }
            }
        }

        field_node.children = child_ids;
    }

    /// Parse a value line (continuation line for a field value).
    fn parse_value_line(&mut self) -> NodeId {
        let tok = self.peek().clone();
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::ValueLine, tok.span);
        node.leading_trivia = tok.leading_trivia;
        node.content_span = tok.span;
        node.indent = tok.indent;
        self.collect_trailing_newline(&mut node);
        self.finalize_node_span(&mut node);
        self.alloc_node(node)
    }

    /// Parse a field name token as a value line (for cases where a field-like
    /// token appears as a continuation).
    fn parse_value_line_from_field(&mut self) -> NodeId {
        // Consume FieldName, Colon, and optional Value as one ValueLine.
        let name_tok = self.peek().clone();
        self.pos += 1;
        let mut end = name_tok.span.end;

        // Consume colon if present.
        if !self.at_eof() && self.peek().kind == TokenKind::Colon {
            end = self.peek().span.end;
            self.pos += 1;
        }

        // Consume value if present on same line.
        if !self.at_eof() && self.peek().kind == TokenKind::Value {
            let has_newline = self
                .peek()
                .leading_trivia
                .iter()
                .any(|t| t.kind == TriviaKind::Newline);
            if !has_newline {
                end = self.peek().span.end;
                self.pos += 1;
            }
        }

        let content_span = Span::new(name_tok.span.start, end);
        let mut node = CstNode::new(CstNodeKind::ValueLine, content_span);
        node.leading_trivia = name_tok.leading_trivia;
        node.content_span = content_span;
        node.indent = name_tok.indent;
        self.collect_trailing_newline(&mut node);
        self.finalize_node_span(&mut node);
        self.alloc_node(node)
    }

    /// Parse an `import: stanza-name` directive.
    fn parse_import(&mut self) -> NodeId {
        // Same shape as a field, but with Import kind.
        let name_tok = self.peek().clone();
        debug_assert_eq!(name_tok.kind, TokenKind::FieldName);
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::Import, name_tok.span);
        node.leading_trivia = name_tok.leading_trivia;
        node.field_name = Some(name_tok.span);
        node.indent = name_tok.indent;

        let mut content_end = name_tok.span.end;

        // Colon.
        if !self.at_eof() && self.peek().kind == TokenKind::Colon {
            content_end = self.peek().span.end;
            self.pos += 1;

            // Value (stanza name).
            if !self.at_eof() && self.peek().kind == TokenKind::Value {
                let has_newline = self
                    .peek()
                    .leading_trivia
                    .iter()
                    .any(|t| t.kind == TriviaKind::Newline);
                if !has_newline {
                    let val_tok = self.peek().clone();
                    node.field_value = Some(val_tok.span);
                    content_end = val_tok.span.end;
                    self.pos += 1;
                }
            }
        }

        node.content_span = Span::new(name_tok.span.start, content_end);
        self.collect_trailing_newline(&mut node);
        self.finalize_node_span(&mut node);
        self.alloc_node(node)
    }

    /// Parse a section: `library`, `executable foo`, etc.
    /// Also handles braced layout: `library { ... }`, `executable foo { ... }`.
    fn parse_section(&mut self) -> NodeId {
        let header_tok = self.peek().clone();
        debug_assert_eq!(header_tok.kind, TokenKind::SectionHeader);
        let section_indent = header_tok.indent;
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::Section, header_tok.span);
        node.leading_trivia = header_tok.leading_trivia;
        node.section_keyword = Some(header_tok.span);
        node.indent = section_indent;

        let mut content_end = header_tok.span.end;

        // Section argument (e.g. `my-exe` in `executable my-exe`).
        // Note: for `library {`, the lexer emits SectionArg("{").
        // For `executable foo {`, the lexer emits SectionArg("foo {").
        if !self.at_eof() && self.peek().kind == TokenKind::SectionArg {
            let arg_tok = self.peek().clone();
            node.section_arg = Some(arg_tok.span);
            content_end = arg_tok.span.end;
            self.pos += 1;
        }

        // Check for braced layout on the same line: the SectionArg ends with `{`.
        // E.g. `library {` → SectionArg is "{", or `executable foo {` → SectionArg is "foo {".
        let is_braced_same_line = node.section_arg.is_some_and(|arg_span| {
            let arg_text = arg_span.slice(&self.source);
            arg_text.trim_end().ends_with('{')
        });

        // Adjust the section_arg span if it contains a trailing `{`.
        if is_braced_same_line {
            if let Some(arg_span) = node.section_arg {
                let arg_text = arg_span.slice(&self.source);
                let trimmed = arg_text.trim_end();
                // Remove the trailing `{` and any whitespace before it.
                let without_brace = trimmed.trim_end_matches('{').trim_end();
                if without_brace.is_empty() {
                    // The entire arg was just `{` — no real section name.
                    node.section_arg = None;
                } else {
                    // Trim the arg span to exclude the `{` and preceding whitespace.
                    let new_end = arg_span.start + without_brace.len();
                    node.section_arg = Some(Span::new(arg_span.start, new_end));
                }
            }
        }

        // Check for braced layout on the next line: a Value token containing `{`.
        let is_braced_next_line = !is_braced_same_line
            && !self.at_eof()
            && self.peek().kind == TokenKind::Value
            && self.peek().span.slice(&self.source).trim() == "{";

        if is_braced_next_line {
            // Include the `{` token in the content span.
            content_end = self.peek().span.end;
            self.pos += 1;
        }

        let is_braced = is_braced_same_line || is_braced_next_line;

        node.content_span = Span::new(header_tok.span.start, content_end);

        // Collect trailing newline for the header line.
        self.collect_trailing_newline(&mut node);

        // Allocate the section node now so children can reference it.
        let section_id = self.alloc_node(node);

        if is_braced {
            // Parse braced section body: consume children until `}`.
            self.parse_braced_body(section_id);
        } else {
            // Parse section body: items indented more than the section header.
            self.parse_body(section_id, section_indent, false);
        }

        // Update the section's span to cover its entire body.
        let body_end = self.last_child_end(section_id);
        self.nodes[section_id.0].span = Span::new(self.nodes[section_id.0].span.start, body_end);

        section_id
    }

    /// Parse a conditional: `if condition` + body, optional `else` + body.
    /// Also handles braced layout: `if flag(dev) { ... }`.
    fn parse_conditional(&mut self, cond_indent: usize) -> NodeId {
        let kw_tok = self.peek().clone();
        debug_assert!(matches!(kw_tok.kind, TokenKind::If | TokenKind::Elif));
        self.pos += 1;

        let mut node = CstNode::new(CstNodeKind::Conditional, kw_tok.span);
        node.leading_trivia = kw_tok.leading_trivia;
        node.condition_keyword = Some(kw_tok.span);
        node.indent = cond_indent;

        // Consume the condition expression tokens until we hit a newline.
        // The condition is everything on the same line after the keyword.
        // Note: consume_condition_expr stops before a `{` Value token.
        let expr_start = self.find_condition_expr_start();
        let expr_end = self.consume_condition_expr();

        if expr_start < expr_end {
            node.condition_expr = Some(Span::new(expr_start, expr_end));
        }

        // Check for braced layout on the same line: `{` Value token remaining.
        let is_braced_same_line = !self.at_eof()
            && self.peek().kind == TokenKind::Value
            && !self
                .peek()
                .leading_trivia
                .iter()
                .any(|t| t.kind == TriviaKind::Newline)
            && self.peek().span.slice(&self.source).trim() == "{";

        let mut content_end = if expr_end > kw_tok.span.end {
            expr_end
        } else {
            kw_tok.span.end
        };

        if is_braced_same_line {
            // Consume the `{` token — include it in the content span.
            content_end = self.peek().span.end;
            self.pos += 1;
        }

        // Check for braced layout on the next line: a Value token `{` on a new line.
        let is_braced_next_line = !is_braced_same_line
            && !self.at_eof()
            && self.peek().kind == TokenKind::Value
            && self.peek().span.slice(&self.source).trim() == "{";

        if is_braced_next_line {
            content_end = self.peek().span.end;
            self.pos += 1;
        }

        let is_braced = is_braced_same_line || is_braced_next_line;

        node.content_span = Span::new(kw_tok.span.start, content_end);

        // Collect trailing newline.
        self.collect_trailing_newline(&mut node);

        // Allocate the conditional node.
        let cond_id = self.alloc_node(node);

        if is_braced {
            // Parse braced then-block: consume children until `}`.
            self.parse_braced_body(cond_id);
        } else {
            // Parse then-block: items indented more than the conditional.
            self.parse_body(cond_id, cond_indent, false);
        }

        // Check for `else` at the same indent level.
        if !self.at_eof()
            && self.peek().kind == TokenKind::Else
            && self.peek().indent == cond_indent
        {
            let else_id = self.parse_else_block(cond_indent);
            self.add_child(cond_id, else_id);
        }

        // Update span.
        let body_end = self.last_child_end(cond_id);
        self.nodes[cond_id.0].span = Span::new(self.nodes[cond_id.0].span.start, body_end);

        cond_id
    }

    /// Parse an `else` block.
    fn parse_else_block(&mut self, cond_indent: usize) -> NodeId {
        let else_tok = self.peek().clone();
        debug_assert_eq!(else_tok.kind, TokenKind::Else);
        self.pos += 1;

        // Check for braced else: `else {` — a Value token with `{` on the
        // same line as the `else` keyword.
        let is_braced = !self.at_eof()
            && self.peek().kind == TokenKind::Value
            && !self
                .peek()
                .leading_trivia
                .iter()
                .any(|t| t.kind == TriviaKind::Newline)
            && self.peek().span.slice(&self.source).trim() == "{";

        let content_end = if is_braced {
            // Consume the `{` token — include it in the else block's content span.
            let brace_tok = self.peek().clone();
            let _ = brace_tok; // consumed below
            let end = self.peek().span.end;
            self.pos += 1;
            end
        } else {
            else_tok.span.end
        };

        let mut node = CstNode::new(CstNodeKind::ElseBlock, else_tok.span);
        node.leading_trivia = else_tok.leading_trivia;
        node.content_span = Span::new(else_tok.span.start, content_end);
        node.indent = else_tok.indent;

        // Trailing newline.
        self.collect_trailing_newline(&mut node);

        // Allocate.
        let else_id = self.alloc_node(node);

        if is_braced {
            // Parse braced else body: consume children until we see `}`.
            self.parse_braced_body(else_id);
        } else {
            // Parse else body using indentation.
            self.parse_body(else_id, cond_indent, false);
        }

        // Update span.
        let body_end = self.last_child_end(else_id);
        self.nodes[else_id.0].span = Span::new(self.nodes[else_id.0].span.start, body_end);

        else_id
    }

    /// Parse a braced block body (`{ ... }`). Consumes children until we see
    /// a Value token that is just `}`. The closing `}` is consumed and added
    /// as a ValueLine child so it appears in the rendered output.
    fn parse_braced_body(&mut self, parent_id: NodeId) {
        loop {
            if self.at_eof() {
                break;
            }

            let tok = self.peek();

            // Check for the closing `}`.
            if tok.kind == TokenKind::Value && tok.span.slice(&self.source).trim() == "}" {
                let node_id = self.parse_value_line();
                self.add_child(parent_id, node_id);
                break;
            }

            match tok.kind {
                TokenKind::Comment => {
                    let node_id = self.parse_comment();
                    self.add_child(parent_id, node_id);
                }
                TokenKind::SectionHeader => {
                    let node_id = self.parse_section();
                    self.add_child(parent_id, node_id);
                }
                TokenKind::If | TokenKind::Elif => {
                    let indent = tok.indent;
                    let node_id = self.parse_conditional(indent);
                    self.add_child(parent_id, node_id);
                }
                TokenKind::Else => {
                    // Standalone else inside braced block — treat as error but
                    // keep going.
                    break;
                }
                TokenKind::FieldName => {
                    let is_import = {
                        let name_text = tok.span.slice(&self.source);
                        name_text.eq_ignore_ascii_case("import")
                    };
                    if is_import {
                        let node_id = self.parse_import();
                        self.add_child(parent_id, node_id);
                    } else {
                        let indent = tok.indent;
                        let node_id = self.parse_field(indent);
                        self.add_child(parent_id, node_id);
                    }
                }
                TokenKind::Value => {
                    let node_id = self.parse_value_line();
                    self.add_child(parent_id, node_id);
                }
                TokenKind::Eof => break,
                _ => {
                    let span = tok.span;
                    let kind = tok.kind;
                    self.emit_error(span, format!("unexpected token in braced block: {kind:?}"));
                    self.pos += 1;
                }
            }
        }
    }

    // -- Condition expression helpers ----------------------------------------

    /// Find the byte offset where the condition expression starts (skipping
    /// whitespace trivia tokens after the keyword).
    fn find_condition_expr_start(&self) -> usize {
        if self.at_eof() {
            return self.source.len();
        }
        let tok = self.peek();
        // The expression starts at the first non-trivia content after the keyword.
        // The token's leading trivia includes whitespace, so the expr starts at
        // the token's span start.
        tok.span.start
    }

    /// Consume condition expression tokens (everything on the same line after
    /// the `if` / `elif` keyword). Returns the end byte offset.
    ///
    /// Stops before consuming a trailing `{` Value token so that the caller
    /// can detect braced layout.
    fn consume_condition_expr(&mut self) -> usize {
        let mut end = 0;
        // Consume tokens until we see one whose leading trivia contains a
        // newline (meaning it's on the next line) or we hit EOF.
        loop {
            if self.at_eof() {
                break;
            }
            let tok = self.peek();
            // Check if this token is on a new line.
            let has_newline = tok
                .leading_trivia
                .iter()
                .any(|t| t.kind == TriviaKind::Newline);
            if has_newline {
                break;
            }
            // Stop before a `{` Value token — it signals braced layout.
            if tok.kind == TokenKind::Value && tok.span.slice(&self.source).trim() == "{" {
                break;
            }
            // Token is on the same line — it's part of the condition expr.
            end = tok.span.end;
            self.pos += 1;
        }
        end
    }

    // -- Trivia helpers -----------------------------------------------------

    /// If the next pending trivia (on the next token) starts with a Newline,
    /// steal it and add to this node's trailing trivia. This handles the
    /// common case where a node's line ends with `\n` and that newline should
    /// belong to this node, not the next one.
    fn collect_trailing_newline(&mut self, node: &mut CstNode) {
        if self.pos >= self.tokens.len() {
            return;
        }
        let next_tok = &mut self.tokens[self.pos];
        // Collect leading newlines (and any whitespace before them that's
        // actually the remainder of the current line — but our lexer puts
        // newlines at the start of the next token's trivia).
        let mut stolen = Vec::new();
        let mut remaining = Vec::new();
        let mut found_newline = false;

        for tp in next_tok.leading_trivia.drain(..) {
            if !found_newline && tp.kind == TriviaKind::Newline {
                stolen.push(tp);
                found_newline = true;
            } else if found_newline {
                remaining.push(tp);
            } else {
                // Trivia before the newline (shouldn't normally happen since
                // newlines are first in the trivia of the next token, but
                // just in case).
                stolen.push(tp);
            }
        }

        next_tok.leading_trivia = remaining;
        node.trailing_trivia.extend(stolen);
    }

    /// Update a node's `span` to cover its leading trivia, content, and
    /// trailing trivia.
    fn finalize_node_span(&self, node: &mut CstNode) {
        let start = node
            .leading_trivia
            .first()
            .map(|t| t.span.start)
            .unwrap_or(node.content_span.start);
        let end = node
            .trailing_trivia
            .last()
            .map(|t| t.span.end)
            .unwrap_or(node.content_span.end);
        node.span = Span::new(start, end);
    }

    /// Get the end byte offset of the last child of a node (or the node's
    /// own span end if it has no children).
    fn last_child_end(&self, node_id: NodeId) -> usize {
        let node = &self.nodes[node_id.0];
        if let Some(&last_child) = node.children.last() {
            self.nodes[last_child.0].span.end
        } else {
            node.span.end
        }
    }
}

/// Parse a `.cabal` source string into a CST with diagnostics.
pub fn parse(source: &str) -> ParseResult {
    let tokens = tokenize(source);
    let parser = Parser::new(source.to_owned(), tokens);
    parser.parse()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse and verify round-trip.
    fn assert_round_trip(source: &str) {
        let result = parse(source);
        let rendered = result.cst.render();
        assert_eq!(
            rendered, source,
            "\n--- EXPECTED ---\n{source}\n--- GOT ---\n{rendered}\n"
        );
    }

    // -- Round-trip tests ---------------------------------------------------

    #[test]
    fn round_trip_minimal() {
        assert_round_trip("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
    }

    #[test]
    fn round_trip_with_comments() {
        assert_round_trip(
            "-- Top comment\ncabal-version: 3.0\nname: foo\n-- A comment\nversion: 0.1.0.0\n",
        );
    }

    #[test]
    fn round_trip_with_blank_lines() {
        assert_round_trip("cabal-version: 3.0\nname: foo\n\nversion: 0.1.0.0\n");
    }

    #[test]
    fn round_trip_section() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base >=4.14
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_section_with_arg() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

executable my-exe
  main-is: Main.hs
  build-depends: base
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_conditional() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base
  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_common_stanza() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules: Foo
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_multiline_field() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules:
    Foo
    Bar
    Baz
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_leading_comma_deps() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_trailing_comma_deps() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_single_line_deps() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_multiple_sections() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base

executable my-exe
  main-is: Main.hs
  build-depends: base, foo

test-suite tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, foo, tasty
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_complex_conditional() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base
  if flag(dev) && !os(windows)
    ghc-options: -O0
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_no_trailing_newline() {
        assert_round_trip("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0");
    }

    #[test]
    fn round_trip_field_extra_spaces() {
        assert_round_trip("name:    foo\nversion:    0.1.0.0\n");
    }

    #[test]
    fn round_trip_comment_in_section() {
        let src = "\
library
  -- A comment in the library
  exposed-modules: Foo
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_blank_line_between_sections() {
        let src = "\
library
  exposed-modules: Foo

executable bar
  main-is: Main.hs
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_flag_section() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

flag dev
  description: Development mode
  default: False
  manual: True
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_source_repository() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

source-repository head
  type: git
  location: https://github.com/example/foo
";
        assert_round_trip(src);
    }

    // -- Structure tests ----------------------------------------------------

    #[test]
    fn parse_structure_simple() {
        let src = "cabal-version: 3.0\nname: foo\n";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        // Root should have 2 field children.
        assert_eq!(root.children.len(), 2);
        for &child_id in &root.children {
            assert_eq!(result.cst.node(child_id).kind, CstNodeKind::Field);
        }
    }

    #[test]
    fn parse_structure_section_with_children() {
        let src = "\
library
  exposed-modules: Foo
  build-depends: base
";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        assert_eq!(root.children.len(), 1);
        let section = result.cst.node(root.children[0]);
        assert_eq!(section.kind, CstNodeKind::Section);
        assert_eq!(section.section_keyword.unwrap().slice(&src), "library");
        assert!(section.section_arg.is_none());
        assert_eq!(section.children.len(), 2);
    }

    #[test]
    fn parse_structure_section_with_arg() {
        let src = "executable my-exe\n  main-is: Main.hs\n";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let section = result.cst.node(root.children[0]);
        assert_eq!(section.kind, CstNodeKind::Section);
        assert_eq!(section.section_keyword.unwrap().slice(&src), "executable");
        assert_eq!(section.section_arg.unwrap().slice(&src), "my-exe");
    }

    #[test]
    fn parse_structure_conditional() {
        let src = "\
library
  build-depends: base
  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2
";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let section = result.cst.node(root.children[0]);
        // Section children: field (build-depends) + conditional.
        assert_eq!(section.children.len(), 2);
        let cond = result.cst.node(section.children[1]);
        assert_eq!(cond.kind, CstNodeKind::Conditional);
        // Conditional children: then-block field + else block.
        assert!(cond.children.len() >= 2);
        // Last child should be ElseBlock.
        let last = result.cst.node(*cond.children.last().unwrap());
        assert_eq!(last.kind, CstNodeKind::ElseBlock);
    }

    #[test]
    fn parse_structure_import() {
        let src = "\
library
  import: warnings
  exposed-modules: Foo
";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let section = result.cst.node(root.children[0]);
        let import = result.cst.node(section.children[0]);
        assert_eq!(import.kind, CstNodeKind::Import);
        assert_eq!(import.field_name.unwrap().slice(&src), "import");
        assert_eq!(import.field_value.unwrap().slice(&src), "warnings");
    }

    #[test]
    fn parse_structure_multiline_field() {
        let src = "\
library
  exposed-modules:
    Foo
    Bar
";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let section = result.cst.node(root.children[0]);
        let field = result.cst.node(section.children[0]);
        assert_eq!(field.kind, CstNodeKind::Field);
        assert_eq!(field.field_name.unwrap().slice(&src), "exposed-modules");
        // Should have 2 ValueLine children.
        assert_eq!(field.children.len(), 2);
        for &child_id in &field.children {
            assert_eq!(result.cst.node(child_id).kind, CstNodeKind::ValueLine);
        }
    }

    #[test]
    fn parse_no_diagnostics_for_valid_file() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules:
    Foo
    Bar
  build-depends:
    base >=4.14
  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2

executable my-exe
  import: warnings
  main-is: Main.hs
  build-depends: base, foo
";
        let result = parse(src);
        assert!(
            result.diagnostics.is_empty(),
            "expected no diagnostics, got: {:?}",
            result.diagnostics
        );
    }

    // -- Error recovery tests -----------------------------------------------

    #[test]
    fn parse_does_not_panic_on_empty_input() {
        let result = parse("");
        assert!(result.cst.render().is_empty());
    }

    #[test]
    fn parse_does_not_panic_on_blank_lines_only() {
        let src = "\n\n\n";
        let result = parse(src);
        assert_eq!(result.cst.render(), src);
    }

    #[test]
    fn parse_does_not_panic_on_comments_only() {
        let src = "-- just a comment\n-- another one\n";
        let result = parse(src);
        assert_eq!(result.cst.render(), src);
    }

    // -- Field name/value span tests ----------------------------------------

    #[test]
    fn field_name_and_value_spans() {
        let src = "name: foo\n";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let field = result.cst.node(root.children[0]);
        assert_eq!(field.field_name.unwrap().slice(&src), "name");
        assert_eq!(field.field_value.unwrap().slice(&src), "foo");
    }

    #[test]
    fn field_no_value() {
        let src = "build-depends:\n";
        let result = parse(src);
        let root = result.cst.node(result.cst.root);
        let field = result.cst.node(root.children[0]);
        assert_eq!(field.field_name.unwrap().slice(&src), "build-depends");
        assert!(field.field_value.is_none());
    }

    // -- Large round-trip tests ---------------------------------------------

    #[test]
    fn round_trip_realistic_file() {
        let src = "\
cabal-version:   3.0
name:            my-project
version:         0.1.0.0
synopsis:        A sample project
description:
  This is a longer description
  that spans multiple lines.
license:         MIT
license-file:    LICENSE
author:          Test Author
maintainer:      test@example.com
category:        Development
build-type:      Simple

common warnings
  ghc-options: -Wall -Wcompat -Widentities
               -Wincomplete-record-updates
               -Wincomplete-uni-patterns
               -Wmissing-deriving-strategies
               -Wredundant-constraints

flag dev
  description: Enable development mode
  default:     False
  manual:      True

library
  import:           warnings
  exposed-modules:
    MyProject
    MyProject.Internal
    MyProject.Types
  other-modules:
    MyProject.Utils
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , text >=2.0 && <2.2
    , containers ^>=0.6
  hs-source-dirs:   src
  default-language: GHC2021
  default-extensions:
    OverloadedStrings
    DerivingStrategies

  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2

executable my-project
  import:           warnings
  main-is:          Main.hs
  other-modules:    Paths_my_project
  build-depends:
      base
    , my-project
    , optparse-applicative ^>=0.18
  hs-source-dirs:   app
  default-language: GHC2021

test-suite my-project-test
  import:           warnings
  type:             exitcode-stdio-1.0
  main-is:          Main.hs
  other-modules:
    Test.MyProject
    Test.MyProject.Types
  build-depends:
      base
    , my-project
    , tasty ^>=1.5
    , tasty-hunit ^>=0.10
  hs-source-dirs:   test
  default-language: GHC2021

source-repository head
  type:     git
  location: https://github.com/example/my-project
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_nested_conditionals() {
        let src = "\
library
  build-depends: base
  if os(linux)
    if flag(dbus)
      build-depends: dbus
      cpp-options: -DDBUS
  if os(windows)
    build-depends: Win32
";
        assert_round_trip(src);
    }

    #[test]
    fn round_trip_benchmark_section() {
        let src = "\
benchmark my-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, criterion
  hs-source-dirs: bench
";
        assert_round_trip(src);
    }

    // -- Braced layout tests --------------------------------------------------

    #[test]
    fn round_trip_braced_section() {
        assert_round_trip("library {\n  exposed-modules: Foo\n  build-depends: base\n}\n");
    }

    #[test]
    fn round_trip_braced_executable() {
        assert_round_trip("executable foo {\n  main-is: Main.hs\n}\n");
    }

    #[test]
    fn round_trip_braced_if() {
        assert_round_trip(
            "library\n  build-depends: base\n  if flag(dev) {\n    ghc-options: -O0\n  }\n",
        );
    }

    #[test]
    fn round_trip_braced_if_else() {
        assert_round_trip(
            "library\n  if flag(dev) {\n    ghc-options: -O0\n  } else {\n    ghc-options: -O2\n  }\n",
        );
    }
}
