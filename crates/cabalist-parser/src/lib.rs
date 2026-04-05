//! # cabalist-parser
//!
//! A Rust library that parses `.cabal` files into a concrete syntax tree (CST),
//! preserving every byte of the original source — whitespace, comments, blank
//! lines, indentation style. The CST can be rendered back to text with
//! byte-identical round-tripping when no edits have been made.
//!
//! The parser maintains two representations:
//!
//! - **CST (Concrete Syntax Tree):** A flat-arena tree that captures every byte
//!   of the source. This is what gets mutated during edits and rendered back to
//!   text.
//!
//! - **AST (Abstract Syntax Tree):** A typed, ergonomic view derived from the
//!   CST for querying and validation. *(Implemented in a separate module.)*
//!
//! ## Quick start
//!
//! ```
//! use cabalist_parser::parse;
//!
//! let source = "cabal-version: 3.0\nname: my-package\nversion: 0.1.0.0\n";
//! let result = parse(source);
//!
//! // No diagnostics for a valid file.
//! assert!(result.diagnostics.is_empty());
//!
//! // Round-trip: render produces the original source.
//! assert_eq!(result.cst.render(), source);
//! ```

/// Typed abstract syntax tree derived from the CST.
pub mod ast;
/// Concrete syntax tree with byte-identical round-trip fidelity.
pub mod cst;
/// Parser diagnostics (errors and warnings).
pub mod diagnostic;
/// CST mutation operations (add, remove, update fields).
pub mod edit;
/// Lexer that tokenizes `.cabal` source into CST nodes.
pub mod lexer;
/// Parser that builds a CST from source text.
pub mod parse;
/// Byte spans and node identifiers.
pub mod span;
/// Structural validation of parsed `.cabal` files.
pub mod validate;

// Re-export the main entry point and key types at the crate root.
pub use ast::{CabalFile, Dependency, Version, VersionRange};
pub use cst::{CabalCst, CstNode, CstNodeKind};
pub use diagnostic::{Diagnostic, Severity};
pub use edit::{EditBatch, ListStyle, TextEdit};
pub use parse::ParseResult;
pub use span::{NodeId, Span};
pub use validate::validate;

/// Parse a `.cabal` source string into a [`ParseResult`] containing the CST
/// and any diagnostics.
///
/// The returned CST preserves every byte of the input. Calling
/// [`CabalCst::render()`] on an unmodified CST produces byte-identical output.
pub fn parse(source: &str) -> ParseResult {
    parse::parse(source)
}
