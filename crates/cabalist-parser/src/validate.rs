//! Validation of a parsed `.cabal` file against the Cabal specification.
//!
//! These are **objective spec violations**, not opinionated lints. Opinionated
//! checks belong in the separate `cabalist-opinions` crate.

use std::collections::{HashMap, HashSet};

use crate::cst::{CabalCst, CstNodeKind};
use crate::diagnostic::Diagnostic;
use crate::span::{NodeId, Span};

/// Validate a parsed CST against the `.cabal` specification.
/// Returns diagnostics for any spec violations found.
pub fn validate(cst: &CabalCst) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let ctx = ValidationContext::collect(cst);

    check_required_fields(cst, &ctx, &mut diagnostics);
    check_cabal_version_first(cst, &ctx, &mut diagnostics);
    check_cabal_version_value(cst, &ctx, &mut diagnostics);
    check_duplicate_top_level_fields(cst, &ctx, &mut diagnostics);
    check_duplicate_section_fields(cst, &ctx, &mut diagnostics);
    check_duplicate_sections(cst, &ctx, &mut diagnostics);
    check_import_references(cst, &ctx, &mut diagnostics);
    check_build_type(cst, &ctx, &mut diagnostics);
    check_library_exposed_modules(cst, &ctx, &mut diagnostics);

    diagnostics
}

// ---------------------------------------------------------------------------
// Field name canonicalization
// ---------------------------------------------------------------------------

/// Canonicalize a field name: lowercase, replace underscores with hyphens.
fn canonicalize_field_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

// ---------------------------------------------------------------------------
// Helper: extract field value text from a CST node
// ---------------------------------------------------------------------------

/// Get the trimmed text of a field's value from the CST.
fn get_field_value(cst: &CabalCst, node_id: NodeId) -> Option<&str> {
    let node = cst.node(node_id);
    node.field_value.map(|span| span.slice(&cst.source).trim())
}

// ---------------------------------------------------------------------------
// Collected information about the CST for validation
// ---------------------------------------------------------------------------

/// A field occurrence: its canonical name, the span of the field name, and the
/// node id.
#[derive(Debug)]
struct FieldInfo {
    canonical_name: String,
    name_span: Span,
    node_id: NodeId,
}

/// A section occurrence: its keyword, optional argument (name), and the node id.
#[derive(Debug)]
struct SectionInfo {
    keyword: String,
    arg: Option<String>,
    keyword_span: Span,
    node_id: NodeId,
}

/// Pre-collected information from a single CST walk, used by all validation
/// checks.
#[derive(Debug)]
struct ValidationContext {
    /// Top-level fields (direct children of Root that are Field nodes).
    top_level_fields: Vec<FieldInfo>,
    /// Top-level sections (direct children of Root that are Section nodes).
    sections: Vec<SectionInfo>,
    /// Names of all `common` stanzas.
    common_stanza_names: HashSet<String>,
    /// All `import:` directives: (value text, span of the value, node id).
    imports: Vec<(String, Span, NodeId)>,
    /// Per-section field lists: section NodeId -> Vec<FieldInfo>.
    section_fields: HashMap<usize, Vec<FieldInfo>>,
}

impl ValidationContext {
    /// Walk the CST once and collect all information needed for validation.
    fn collect(cst: &CabalCst) -> Self {
        let mut ctx = ValidationContext {
            top_level_fields: Vec::new(),
            sections: Vec::new(),
            common_stanza_names: HashSet::new(),
            imports: Vec::new(),
            section_fields: HashMap::new(),
        };

        let root = cst.node(cst.root);
        for &child_id in &root.children {
            let child = cst.node(child_id);
            match child.kind {
                CstNodeKind::Field => {
                    if let Some(name_span) = child.field_name {
                        ctx.top_level_fields.push(FieldInfo {
                            canonical_name: canonicalize_field_name(name_span.slice(&cst.source)),
                            name_span,
                            node_id: child_id,
                        });
                    }
                }
                CstNodeKind::Section => {
                    let keyword = child
                        .section_keyword
                        .map(|s| s.slice(&cst.source).to_ascii_lowercase())
                        .unwrap_or_default();
                    let arg = child.section_arg.map(|s| s.slice(&cst.source).to_string());

                    let keyword_span = child.section_keyword.unwrap_or(child.content_span);

                    if keyword == "common" {
                        if let Some(ref name) = arg {
                            ctx.common_stanza_names.insert(name.clone());
                        }
                    }

                    ctx.sections.push(SectionInfo {
                        keyword: keyword.clone(),
                        arg,
                        keyword_span,
                        node_id: child_id,
                    });

                    // Collect fields and imports within this section.
                    ctx.collect_section_children(cst, child_id);
                }
                _ => {}
            }
        }

        ctx
    }

    /// Recursively collect fields and imports from a section (and its
    /// conditionals).
    fn collect_section_children(&mut self, cst: &CabalCst, section_id: NodeId) {
        let section = cst.node(section_id);
        for &child_id in &section.children {
            let child = cst.node(child_id);
            match child.kind {
                CstNodeKind::Field => {
                    if let Some(name_span) = child.field_name {
                        self.section_fields
                            .entry(section_id.0)
                            .or_default()
                            .push(FieldInfo {
                                canonical_name: canonicalize_field_name(
                                    name_span.slice(&cst.source),
                                ),
                                name_span,
                                node_id: child_id,
                            });
                    }
                }
                CstNodeKind::Import => {
                    if let Some(val_span) = child.field_value {
                        let value = val_span.slice(&cst.source).trim().to_string();
                        self.imports.push((value, val_span, child_id));
                    }
                }
                CstNodeKind::Section => {
                    // The parser may nest top-level sections inside each other
                    // when they're at indent 0 (a known parser quirk). Treat
                    // nested Section nodes as additional top-level sections.
                    let keyword = child
                        .section_keyword
                        .map(|s| s.slice(&cst.source).to_ascii_lowercase())
                        .unwrap_or_default();
                    let arg = child.section_arg.map(|s| s.slice(&cst.source).to_string());
                    let keyword_span = child.section_keyword.unwrap_or(child.content_span);

                    if keyword == "common" {
                        if let Some(ref name) = arg {
                            self.common_stanza_names.insert(name.clone());
                        }
                    }

                    self.sections.push(SectionInfo {
                        keyword,
                        arg,
                        keyword_span,
                        node_id: child_id,
                    });

                    // Recurse into this nested section's children.
                    self.collect_section_children(cst, child_id);
                }
                CstNodeKind::Conditional => {
                    // Recurse into the then-block children.
                    self.collect_conditional_children(cst, child_id);
                }
                _ => {}
            }
        }
    }

    /// Collect fields and imports from conditional blocks (then + else).
    fn collect_conditional_children(&mut self, cst: &CabalCst, cond_id: NodeId) {
        let cond = cst.node(cond_id);
        for &child_id in &cond.children {
            let child = cst.node(child_id);
            match child.kind {
                CstNodeKind::Field => {
                    // Fields in conditionals don't count for duplicate-field
                    // checks at the section level (they're conditional), but
                    // we still need to track imports.
                }
                CstNodeKind::Import => {
                    if let Some(val_span) = child.field_value {
                        let value = val_span.slice(&cst.source).trim().to_string();
                        self.imports.push((value, val_span, child_id));
                    }
                }
                CstNodeKind::ElseBlock => {
                    // Recurse into else block children.
                    let else_node = cst.node(child_id);
                    for &else_child_id in &else_node.children {
                        let else_child = cst.node(else_child_id);
                        if else_child.kind == CstNodeKind::Import {
                            if let Some(val_span) = else_child.field_value {
                                let value = val_span.slice(&cst.source).trim().to_string();
                                self.imports.push((value, val_span, else_child_id));
                            }
                        }
                    }
                }
                CstNodeKind::Conditional => {
                    self.collect_conditional_children(cst, child_id);
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation checks
// ---------------------------------------------------------------------------

/// Check that `name`, `version`, and `cabal-version` fields exist at the top
/// level.
fn check_required_fields(
    _cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required = ["cabal-version", "name", "version"];
    for &field_name in &required {
        let found = ctx
            .top_level_fields
            .iter()
            .any(|f| f.canonical_name == field_name);
        if !found {
            // Use a zero-span at the start of the file since the field is missing.
            diagnostics.push(Diagnostic::error(
                Span::new(0, 0),
                format!("missing required field: `{field_name}`"),
            ));
        }
    }
}

/// Check that `cabal-version` is the first non-comment, non-blank field.
fn check_cabal_version_first(
    cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the first top-level Field node in the CST (skipping comments and
    // blank lines).
    let root = cst.node(cst.root);
    let first_field_id = root.children.iter().find(|&&child_id| {
        let child = cst.node(child_id);
        child.kind == CstNodeKind::Field
    });

    let Some(&first_id) = first_field_id else {
        return; // No fields at all; check_required_fields handles that.
    };

    let first_node = cst.node(first_id);
    let Some(name_span) = first_node.field_name else {
        return;
    };

    let name = canonicalize_field_name(name_span.slice(&cst.source));
    if name != "cabal-version" {
        // Find the cabal-version field to point the diagnostic at it.
        if let Some(cv) = ctx
            .top_level_fields
            .iter()
            .find(|f| f.canonical_name == "cabal-version")
        {
            diagnostics.push(Diagnostic::warning(
                cv.name_span,
                "`cabal-version` should be the first field in the file",
            ));
        }
    }
}

/// Check that the `cabal-version` value is a recognized format.
fn check_cabal_version_value(
    cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(cv_field) = ctx
        .top_level_fields
        .iter()
        .find(|f| f.canonical_name == "cabal-version")
    else {
        return; // Missing field is handled by check_required_fields.
    };

    let Some(raw_value) = get_field_value(cst, cv_field.node_id) else {
        return;
    };

    // Strip the deprecated `>=` prefix if present.
    let version_str = raw_value.strip_prefix(">=").unwrap_or(raw_value).trim();

    // A cabal-version value should look like a version number: digits and dots.
    let is_valid_version =
        !version_str.is_empty() && version_str.chars().all(|c| c.is_ascii_digit() || c == '.');

    if !is_valid_version {
        let val_span = cst
            .node(cv_field.node_id)
            .field_value
            .unwrap_or(cv_field.name_span);
        diagnostics.push(Diagnostic::warning(
            val_span,
            format!("unrecognized `cabal-version` value: `{raw_value}`"),
        ));
    }
}

/// Check for duplicate field names at the top level.
fn check_duplicate_top_level_fields(
    _cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_duplicates_in_field_list(&ctx.top_level_fields, diagnostics);
}

/// Check for duplicate field names within each section.
fn check_duplicate_section_fields(
    _cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for fields in ctx.section_fields.values() {
        check_duplicates_in_field_list(fields, diagnostics);
    }
}

/// Helper: find duplicates in a list of fields and emit warnings.
fn check_duplicates_in_field_list(fields: &[FieldInfo], diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    for field in fields {
        if let Some(&first_span) = seen.get(field.canonical_name.as_str()) {
            let _ = first_span; // We point at the duplicate, not the first.
            diagnostics.push(Diagnostic::warning(
                field.name_span,
                format!("duplicate field: `{}`", field.canonical_name),
            ));
        } else {
            seen.insert(&field.canonical_name, field.name_span);
        }
    }
}

/// Check for duplicate section names (e.g. two `executable foo`).
fn check_duplicate_sections(
    _cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Key: (keyword, optional arg). For unnamed sections like `library`, the
    // arg is None.
    let mut seen: HashMap<(String, Option<String>), Span> = HashMap::new();
    for section in &ctx.sections {
        let key = (section.keyword.clone(), section.arg.clone());
        if let Some(&_first_span) = seen.get(&key) {
            let label = match &section.arg {
                Some(arg) => format!("`{} {}`", section.keyword, arg),
                None => format!("`{}`", section.keyword),
            };
            diagnostics.push(Diagnostic::error(
                section.keyword_span,
                format!("duplicate section: {label}"),
            ));
        } else {
            seen.insert(key, section.keyword_span);
        }
    }
}

/// Check that all `import:` directives reference an existing `common` stanza.
fn check_import_references(
    _cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (value, val_span, _node_id) in &ctx.imports {
        if !ctx.common_stanza_names.contains(value.as_str()) {
            diagnostics.push(Diagnostic::error(
                *val_span,
                format!("import references undefined common stanza: `{value}`"),
            ));
        }
    }
}

/// Check that `build-type` (if present) is one of the valid values.
fn check_build_type(cst: &CabalCst, ctx: &ValidationContext, diagnostics: &mut Vec<Diagnostic>) {
    let Some(bt_field) = ctx
        .top_level_fields
        .iter()
        .find(|f| f.canonical_name == "build-type")
    else {
        return;
    };

    let Some(value) = get_field_value(cst, bt_field.node_id) else {
        return;
    };

    const VALID_BUILD_TYPES: &[&str] = &["Simple", "Configure", "Make", "Custom"];
    if !VALID_BUILD_TYPES.contains(&value) {
        let val_span = cst
            .node(bt_field.node_id)
            .field_value
            .unwrap_or(bt_field.name_span);
        diagnostics.push(Diagnostic::error(
            val_span,
            format!(
                "invalid `build-type` value: `{value}` \
                 (expected one of: Simple, Configure, Make, Custom)"
            ),
        ));
    }
}

/// Check that library sections have `exposed-modules` with at least one
/// module.
fn check_library_exposed_modules(
    cst: &CabalCst,
    ctx: &ValidationContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for section in &ctx.sections {
        if section.keyword != "library" {
            continue;
        }

        let fields = ctx.section_fields.get(&section.node_id.0);

        let has_exposed_modules = fields
            .map(|fs| {
                fs.iter().any(|f| {
                    if f.canonical_name != "exposed-modules" {
                        return false;
                    }
                    // Check if the field has a non-empty value (inline or
                    // via continuation lines).
                    let node = cst.node(f.node_id);
                    let has_inline_value = node
                        .field_value
                        .map(|s| !s.slice(&cst.source).trim().is_empty())
                        .unwrap_or(false);
                    let has_continuation = !node.children.is_empty();
                    has_inline_value || has_continuation
                })
            })
            .unwrap_or(false);

        if !has_exposed_modules {
            diagnostics.push(Diagnostic::warning(
                section.keyword_span,
                "library section has no `exposed-modules` \
                 (or it is empty)",
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    /// Helper: parse source and validate, returning diagnostics.
    fn validate_source(source: &str) -> Vec<Diagnostic> {
        let result = parse(source);
        validate(&result.cst)
    }

    /// Helper: check that a specific message substring appears among
    /// diagnostics.
    fn has_diagnostic(diagnostics: &[Diagnostic], substring: &str) -> bool {
        diagnostics.iter().any(|d| d.message.contains(substring))
    }

    // -- Required fields ---------------------------------------------------

    #[test]
    fn missing_name_field() {
        let diags = validate_source("cabal-version: 3.0\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(&diags, "missing required field: `name`"));
    }

    #[test]
    fn missing_version_field() {
        let diags = validate_source("cabal-version: 3.0\nname: foo\n");
        assert!(has_diagnostic(&diags, "missing required field: `version`"));
    }

    #[test]
    fn missing_cabal_version_field() {
        let diags = validate_source("name: foo\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(
            &diags,
            "missing required field: `cabal-version`"
        ));
    }

    #[test]
    fn all_required_fields_present() {
        let diags = validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
        assert!(
            !has_diagnostic(&diags, "missing required field"),
            "unexpected: {diags:?}"
        );
    }

    // -- cabal-version first ------------------------------------------------

    #[test]
    fn cabal_version_not_first() {
        let diags = validate_source("name: foo\ncabal-version: 3.0\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(
            &diags,
            "`cabal-version` should be the first field"
        ));
    }

    #[test]
    fn cabal_version_is_first() {
        let diags = validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
        assert!(
            !has_diagnostic(&diags, "should be the first field"),
            "unexpected: {diags:?}"
        );
    }

    #[test]
    fn cabal_version_first_after_comments() {
        // Comments before cabal-version are fine.
        let diags =
            validate_source("-- A top comment\ncabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
        assert!(
            !has_diagnostic(&diags, "should be the first field"),
            "unexpected: {diags:?}"
        );
    }

    // -- Duplicate fields ---------------------------------------------------

    #[test]
    fn duplicate_top_level_field() {
        let diags = validate_source("cabal-version: 3.0\nname: foo\nname: bar\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(&diags, "duplicate field: `name`"));
    }

    #[test]
    fn duplicate_field_in_section() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base
  build-depends: text
";
        let diags = validate_source(src);
        assert!(has_diagnostic(&diags, "duplicate field: `build-depends`"));
    }

    #[test]
    fn same_field_different_sections_is_ok() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base

executable bar
  main-is: Main.hs
  build-depends: base
";
        let diags = validate_source(src);
        assert!(
            !has_diagnostic(&diags, "duplicate field"),
            "unexpected: {diags:?}"
        );
    }

    // -- Duplicate sections -------------------------------------------------

    #[test]
    fn duplicate_executable_sections() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

executable bar
  main-is: Main.hs

executable bar
  main-is: Other.hs
";
        let diags = validate_source(src);
        assert!(has_diagnostic(
            &diags,
            "duplicate section: `executable bar`"
        ));
    }

    #[test]
    fn duplicate_unnamed_library() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo

library
  exposed-modules: Bar
";
        let diags = validate_source(src);
        assert!(has_diagnostic(&diags, "duplicate section: `library`"));
    }

    #[test]
    fn different_executable_names_is_ok() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

executable bar
  main-is: Main.hs

executable baz
  main-is: Other.hs
";
        let diags = validate_source(src);
        assert!(
            !has_diagnostic(&diags, "duplicate section"),
            "unexpected: {diags:?}"
        );
    }

    // -- Import references --------------------------------------------------

    #[test]
    fn import_missing_common_stanza() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  import: warnings
  exposed-modules: Foo
";
        let diags = validate_source(src);
        assert!(has_diagnostic(
            &diags,
            "import references undefined common stanza: `warnings`"
        ));
    }

    #[test]
    fn import_with_existing_common_stanza() {
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
        let diags = validate_source(src);
        assert!(
            !has_diagnostic(&diags, "import references undefined"),
            "unexpected: {diags:?}"
        );
    }

    // -- build-type ---------------------------------------------------------

    #[test]
    fn valid_build_type_simple() {
        let diags = validate_source(
            "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\nbuild-type: Simple\n",
        );
        assert!(
            !has_diagnostic(&diags, "invalid `build-type`"),
            "unexpected: {diags:?}"
        );
    }

    #[test]
    fn invalid_build_type() {
        let diags =
            validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\nbuild-type: Foo\n");
        assert!(has_diagnostic(&diags, "invalid `build-type` value: `Foo`"));
    }

    // -- Library exposed-modules --------------------------------------------

    #[test]
    fn library_without_exposed_modules() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base
";
        let diags = validate_source(src);
        assert!(has_diagnostic(
            &diags,
            "library section has no `exposed-modules`"
        ));
    }

    #[test]
    fn library_with_exposed_modules() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base
";
        let diags = validate_source(src);
        assert!(
            !has_diagnostic(&diags, "exposed-modules"),
            "unexpected: {diags:?}"
        );
    }

    #[test]
    fn library_with_multiline_exposed_modules() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules:
    Foo
    Bar
  build-depends: base
";
        let diags = validate_source(src);
        assert!(
            !has_diagnostic(&diags, "exposed-modules"),
            "unexpected: {diags:?}"
        );
    }

    // -- cabal-version value ------------------------------------------------

    #[test]
    fn cabal_version_valid_value() {
        let diags = validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
        assert!(
            !has_diagnostic(&diags, "unrecognized `cabal-version`"),
            "unexpected: {diags:?}"
        );
    }

    #[test]
    fn cabal_version_deprecated_prefix() {
        // The `>=` prefix is deprecated but the version itself is valid; no warning.
        let diags = validate_source("cabal-version: >=1.10\nname: foo\nversion: 0.1.0.0\n");
        assert!(
            !has_diagnostic(&diags, "unrecognized `cabal-version`"),
            "unexpected: {diags:?}"
        );
    }

    #[test]
    fn cabal_version_invalid_value() {
        let diags = validate_source("cabal-version: foobar\nname: foo\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(&diags, "unrecognized `cabal-version` value"));
    }

    // -- Full valid file (zero diagnostics) ---------------------------------

    #[test]
    fn full_valid_file_no_diagnostics() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0
synopsis: A test package
build-type: Simple

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules: Foo
  build-depends: base >=4.14

executable my-exe
  import: warnings
  main-is: Main.hs
  build-depends: base, foo

test-suite tests
  import: warnings
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, foo, tasty
";
        let diags = validate_source(src);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // -- Case/underscore insensitivity for duplicate detection ---------------

    #[test]
    fn duplicate_field_case_insensitive() {
        let diags = validate_source("cabal-version: 3.0\nName: foo\nname: bar\nversion: 0.1.0.0\n");
        assert!(has_diagnostic(&diags, "duplicate field: `name`"));
    }

    #[test]
    fn duplicate_field_underscore_hyphen() {
        let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base
  build_depends: text
";
        let diags = validate_source(src);
        assert!(has_diagnostic(&diags, "duplicate field: `build-depends`"));
    }

    // -- Edge cases ---------------------------------------------------------

    #[test]
    fn empty_file() {
        let diags = validate_source("");
        // Should report all three missing required fields.
        assert!(has_diagnostic(
            &diags,
            "missing required field: `cabal-version`"
        ));
        assert!(has_diagnostic(&diags, "missing required field: `name`"));
        assert!(has_diagnostic(&diags, "missing required field: `version`"));
    }

    #[test]
    fn comments_only_file() {
        let diags = validate_source("-- just a comment\n");
        assert!(has_diagnostic(
            &diags,
            "missing required field: `cabal-version`"
        ));
        assert!(has_diagnostic(&diags, "missing required field: `name`"));
        assert!(has_diagnostic(&diags, "missing required field: `version`"));
    }
}
