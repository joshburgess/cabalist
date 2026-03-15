//! Abstract Syntax Tree (AST) for `.cabal` files.
//!
//! The AST is a typed, ergonomic view derived from the CST. It provides
//! structured access to package metadata, components, dependencies, and
//! conditionals. Every AST node carries a [`NodeId`] back-reference to the
//! corresponding CST node so that edits can be mapped back to the concrete tree.
//!
//! The AST does not own the source text — it borrows from the [`CabalCst`]'s
//! source string.
//!
//! # Usage
//!
//! ```
//! use cabalist_parser::{parse, ast::derive_ast};
//!
//! let source = "cabal-version: 3.0\nname: my-pkg\nversion: 0.1.0.0\n";
//! let result = parse(source);
//! let ast = derive_ast(&result.cst);
//! assert_eq!(ast.name, Some("my-pkg"));
//! ```

use crate::cst::{CabalCst, CstNodeKind};
use crate::span::NodeId;

// ---------------------------------------------------------------------------
// Field name canonicalization
// ---------------------------------------------------------------------------

/// Canonicalize a `.cabal` field name to lowercase with hyphens.
///
/// Field names in `.cabal` files are case-insensitive and treat hyphens and
/// underscores as interchangeable. This function normalizes to the canonical
/// `lowercase-with-hyphens` form.
pub fn canonicalize_field_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

/// A parsed version number like `0.1.0.0`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    pub components: Vec<u64>,
}

impl Version {
    /// Parse a version string such as `"0.1.0.0"` or `"4.14"`.
    ///
    /// Returns `None` if the string is empty or contains non-numeric
    /// components.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let mut components = Vec::new();
        for part in s.split('.') {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            match part.parse::<u64>() {
                Ok(n) => components.push(n),
                Err(_) => return None,
            }
        }
        if components.is_empty() {
            return None;
        }
        Some(Version { components })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for c in &self.components {
            if !first {
                write!(f, ".")?;
            }
            write!(f, "{c}")?;
            first = false;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Version ranges
// ---------------------------------------------------------------------------

/// A version constraint expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRange {
    /// No constraint — any version.
    Any,
    /// No version matches.
    NoVersion,
    /// `==V`
    Eq(Version),
    /// `>V`
    Gt(Version),
    /// `>=V`
    Gte(Version),
    /// `<V`
    Lt(Version),
    /// `<=V`
    Lte(Version),
    /// `^>=V` (PVP major bound)
    MajorBound(Version),
    /// Intersection: `A && B`
    And(Box<VersionRange>, Box<VersionRange>),
    /// Union: `A || B`
    Or(Box<VersionRange>, Box<VersionRange>),
}

impl std::fmt::Display for VersionRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionRange::Any => write!(f, "-any"),
            VersionRange::NoVersion => write!(f, "-none"),
            VersionRange::Eq(v) => write!(f, "=={v}"),
            VersionRange::Gt(v) => write!(f, ">{v}"),
            VersionRange::Gte(v) => write!(f, ">={v}"),
            VersionRange::Lt(v) => write!(f, "<{v}"),
            VersionRange::Lte(v) => write!(f, "<={v}"),
            VersionRange::MajorBound(v) => write!(f, "^>={v}"),
            VersionRange::And(a, b) => write!(f, "{a} && {b}"),
            VersionRange::Or(a, b) => write!(f, "{a} || {b}"),
        }
    }
}

/// Parse a version range string.
///
/// Handles expressions like:
/// - `">=4.14 && <5"`
/// - `"^>=2.2"`
/// - `">=2.0 || ==1.9"`
/// - `">=4.14"`
/// - `"==1.0"`
///
/// Returns `None` if the string is empty or cannot be parsed.
pub fn parse_version_range(s: &str) -> Option<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Split on `||` first (lowest precedence), then `&&`.
    if let Some(range) = parse_or_range(s) {
        return Some(range);
    }

    None
}

/// Parse an `||`-separated version range (lowest precedence).
fn parse_or_range(s: &str) -> Option<VersionRange> {
    // Split on `||` respecting parentheses.
    let parts = split_respecting_parens(s, "||");
    if parts.len() > 1 {
        let mut ranges: Vec<VersionRange> = Vec::new();
        for part in &parts {
            ranges.push(parse_and_range(part.trim())?);
        }
        let mut result = ranges.remove(0);
        for r in ranges {
            result = VersionRange::Or(Box::new(result), Box::new(r));
        }
        return Some(result);
    }
    parse_and_range(s)
}

/// Parse an `&&`-separated version range.
fn parse_and_range(s: &str) -> Option<VersionRange> {
    let parts = split_respecting_parens(s, "&&");
    if parts.len() > 1 {
        let mut ranges: Vec<VersionRange> = Vec::new();
        for part in &parts {
            ranges.push(parse_atom_range(part.trim())?);
        }
        let mut result = ranges.remove(0);
        for r in ranges {
            result = VersionRange::And(Box::new(result), Box::new(r));
        }
        return Some(result);
    }
    parse_atom_range(s)
}

/// Parse a single atomic version range: `>=V`, `<V`, `^>=V`, `==V`, etc.
/// Also handles parenthesized sub-expressions.
fn parse_atom_range(s: &str) -> Option<VersionRange> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // `-any` and `-none` keywords.
    if s.eq_ignore_ascii_case("-any") {
        return Some(VersionRange::Any);
    }
    if s.eq_ignore_ascii_case("-none") {
        return Some(VersionRange::NoVersion);
    }

    // Parenthesized expression.
    if s.starts_with('(') && s.ends_with(')') {
        return parse_or_range(&s[1..s.len() - 1]);
    }

    // `^>=V` or `^>= { v1, v2, v3 }` (set notation)
    if let Some(rest) = s.strip_prefix("^>=") {
        let rest = rest.trim();
        if rest.starts_with('{') && rest.ends_with('}') {
            let inner = &rest[1..rest.len() - 1];
            let versions: Vec<&str> = inner.split(',').map(|v| v.trim()).collect();
            let mut ranges: Vec<VersionRange> = Vec::new();
            for v_str in versions {
                if !v_str.is_empty() {
                    if let Some(v) = Version::parse(v_str) {
                        ranges.push(VersionRange::MajorBound(v));
                    }
                }
            }
            if ranges.is_empty() {
                return None;
            }
            let mut result = ranges.remove(0);
            for r in ranges {
                result = VersionRange::Or(Box::new(result), Box::new(r));
            }
            return Some(result);
        }
        let v = Version::parse(rest)?;
        return Some(VersionRange::MajorBound(v));
    }

    // `>=V`
    if let Some(rest) = s.strip_prefix(">=") {
        let v = Version::parse(rest.trim())?;
        return Some(VersionRange::Gte(v));
    }

    // `<=V`
    if let Some(rest) = s.strip_prefix("<=") {
        let v = Version::parse(rest.trim())?;
        return Some(VersionRange::Lte(v));
    }

    // `==V`, `==V.*`, or `== { v1, v2 }` (set notation)
    if let Some(rest) = s.strip_prefix("==") {
        let rest = rest.trim();
        // Set notation: == { v1, v2, v3 }
        if rest.starts_with('{') && rest.ends_with('}') {
            let inner = &rest[1..rest.len() - 1];
            let versions: Vec<&str> = inner.split(',').map(|v| v.trim()).collect();
            let mut ranges: Vec<VersionRange> = Vec::new();
            for v_str in versions {
                if !v_str.is_empty() {
                    if let Some(v) = Version::parse(v_str) {
                        ranges.push(VersionRange::Eq(v));
                    }
                }
            }
            if ranges.is_empty() {
                return None;
            }
            let mut result = ranges.remove(0);
            for r in ranges {
                result = VersionRange::Or(Box::new(result), Box::new(r));
            }
            return Some(result);
        }
        // Wildcard: ==1.2.* means >= 1.2 && < 1.3
        if let Some(prefix) = rest.strip_suffix(".*") {
            let v = Version::parse(prefix)?;
            let mut upper = v.clone();
            if let Some(last) = upper.components.last_mut() {
                *last += 1;
            }
            return Some(VersionRange::And(
                Box::new(VersionRange::Gte(v)),
                Box::new(VersionRange::Lt(upper)),
            ));
        }
        let v = Version::parse(rest)?;
        return Some(VersionRange::Eq(v));
    }

    // `>V` (must come after `>=`)
    if let Some(rest) = s.strip_prefix('>') {
        let v = Version::parse(rest.trim())?;
        return Some(VersionRange::Gt(v));
    }

    // `<V` (must come after `<=`)
    if let Some(rest) = s.strip_prefix('<') {
        let v = Version::parse(rest.trim())?;
        return Some(VersionRange::Lt(v));
    }

    None
}

/// Split a string on a delimiter, but not inside parentheses.
fn split_respecting_parens<'a>(s: &'a str, delim: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut last = 0;
    let bytes = s.as_bytes();
    let delim_bytes = delim.as_bytes();
    let delim_len = delim_bytes.len();

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            depth += 1;
            i += 1;
        } else if bytes[i] == b')' {
            depth = depth.saturating_sub(1);
            i += 1;
        } else if depth == 0
            && i + delim_len <= bytes.len()
            && &bytes[i..i + delim_len] == delim_bytes
        {
            parts.push(&s[last..i]);
            i += delim_len;
            last = i;
        } else {
            i += 1;
        }
    }
    parts.push(&s[last..]);
    parts
}

// ---------------------------------------------------------------------------
// Dependency
// ---------------------------------------------------------------------------

/// A parsed dependency from `build-depends`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency<'a> {
    /// The package name (e.g. `"aeson"`, `"base"`).
    pub package: &'a str,
    /// The version constraint, if any.
    pub version_range: Option<VersionRange>,
    /// Back-reference to the CST node (the Field or ValueLine containing this
    /// dependency).
    pub cst_node: NodeId,
}

/// Parse a single dependency string like `"aeson ^>=2.2"` or `"base >=4.14 && <5"`.
///
/// The `cst_node` is attached to the resulting `Dependency` for back-reference.
fn parse_single_dependency<'a>(s: &'a str, cst_node: NodeId) -> Option<Dependency<'a>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // The package name is the first token (letters, digits, hyphens).
    let name_end = s
        .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .unwrap_or(s.len());
    let package = s[..name_end].trim();
    if package.is_empty() {
        return None;
    }

    let rest = s[name_end..].trim();
    let version_range = if rest.is_empty() {
        None
    } else {
        parse_version_range(rest)
    };

    Some(Dependency {
        package,
        version_range,
        cst_node,
    })
}

/// Parse a dependency field value (possibly containing multiple comma-separated
/// dependencies) into a vector of [`Dependency`] values.
///
/// Handles:
/// - Single line: `"base >=4.14, text >=2.0, aeson ^>=2.2"`
/// - Individual items from multi-line fields (call once per line).
fn parse_dependencies_from_text<'a>(text: &'a str, cst_node: NodeId) -> Vec<Dependency<'a>> {
    let mut deps = Vec::new();
    // Split on commas.
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(dep) = parse_single_dependency(part, cst_node) {
            deps.push(dep);
        }
    }
    deps
}

// ---------------------------------------------------------------------------
// Cabal version
// ---------------------------------------------------------------------------

/// The `cabal-version` specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CabalVersion<'a> {
    /// The raw text as written in the file.
    pub raw: &'a str,
    /// The parsed version, if it could be parsed.
    pub version: Option<Version>,
    /// Back-reference to the CST field node.
    pub cst_node: NodeId,
}

// ---------------------------------------------------------------------------
// Generic field
// ---------------------------------------------------------------------------

/// A field that was not specifically parsed into a typed representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field<'a> {
    /// The canonicalized field name.
    pub name: String,
    /// The raw field name as written in the source.
    pub raw_name: &'a str,
    /// The field value (first line only; continuation lines are concatenated
    /// with newlines).
    pub value: String,
    /// Back-reference to the CST field node.
    pub cst_node: NodeId,
}

// ---------------------------------------------------------------------------
// Condition AST
// ---------------------------------------------------------------------------

/// A parsed condition expression from `if`/`elif` blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition<'a> {
    /// `flag(name)`
    Flag(&'a str),
    /// `os(name)`
    OS(&'a str),
    /// `arch(name)`
    Arch(&'a str),
    /// `impl(compiler version-range)`
    Impl(&'a str, Option<VersionRange>),
    /// `!condition`
    Not(Box<Condition<'a>>),
    /// `a && b`
    And(Box<Condition<'a>>, Box<Condition<'a>>),
    /// `a || b`
    Or(Box<Condition<'a>>, Box<Condition<'a>>),
    /// Boolean literal: `true` or `false`
    Lit(bool),
    /// Unparsed fallback when the condition couldn't be fully parsed.
    Raw(&'a str),
}

/// Parse a condition expression string.
///
/// Handles expressions like:
/// - `flag(dev)`
/// - `os(windows)`
/// - `flag(dev) && !os(windows)`
/// - `impl(ghc >= 9.6)`
/// - `(flag(a) || flag(b)) && !os(windows)`
pub fn parse_condition(s: &str) -> Condition<'_> {
    let s = s.trim();
    if s.is_empty() {
        return Condition::Raw(s);
    }
    match parse_condition_or(s) {
        Some(c) => c,
        None => Condition::Raw(s),
    }
}

/// Parse `||` (lowest precedence).
fn parse_condition_or(s: &str) -> Option<Condition<'_>> {
    let parts = split_respecting_parens(s, "||");
    if parts.len() > 1 {
        let mut conds: Vec<Condition<'_>> = Vec::new();
        for part in &parts {
            conds.push(parse_condition_and(part.trim())?);
        }
        let mut result = conds.remove(0);
        for c in conds {
            result = Condition::Or(Box::new(result), Box::new(c));
        }
        return Some(result);
    }
    parse_condition_and(s)
}

/// Parse `&&`.
fn parse_condition_and(s: &str) -> Option<Condition<'_>> {
    let parts = split_respecting_parens(s, "&&");
    if parts.len() > 1 {
        let mut conds: Vec<Condition<'_>> = Vec::new();
        for part in &parts {
            conds.push(parse_condition_atom(part.trim())?);
        }
        let mut result = conds.remove(0);
        for c in conds {
            result = Condition::And(Box::new(result), Box::new(c));
        }
        return Some(result);
    }
    parse_condition_atom(s)
}

/// Parse a single condition atom: `!cond`, `flag(name)`, `os(name)`,
/// `arch(name)`, `impl(...)`, or parenthesized sub-expression.
fn parse_condition_atom(s: &str) -> Option<Condition<'_>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Negation.
    if let Some(rest) = s.strip_prefix('!') {
        let inner = parse_condition_atom(rest.trim())?;
        return Some(Condition::Not(Box::new(inner)));
    }

    // Parenthesized.
    if s.starts_with('(') && s.ends_with(')') {
        return parse_condition_or(&s[1..s.len() - 1]);
    }

    // `flag(name)`, `os(name)`, `arch(name)`, `impl(...)`.
    if let Some(paren_start) = s.find('(') {
        if s.ends_with(')') {
            let func = s[..paren_start].trim();
            let arg = s[paren_start + 1..s.len() - 1].trim();
            let func_lower = func.to_ascii_lowercase();
            match func_lower.as_str() {
                "flag" => return Some(Condition::Flag(arg)),
                "os" => return Some(Condition::OS(arg)),
                "arch" => return Some(Condition::Arch(arg)),
                "impl" => {
                    // arg could be e.g. "ghc >= 9.6" or just "ghc".
                    let parts: Vec<&str> = arg.splitn(2, char::is_whitespace).collect();
                    let compiler = parts[0];
                    let vr = if parts.len() > 1 {
                        parse_version_range(parts[1].trim())
                    } else {
                        None
                    };
                    return Some(Condition::Impl(compiler, vr));
                }
                _ => {}
            }
        }
    }

    // Boolean literals
    match s.to_ascii_lowercase().as_str() {
        "true" => return Some(Condition::Lit(true)),
        "false" => return Some(Condition::Lit(false)),
        _ => {}
    }

    // Could not parse — return Raw for the whole string.
    Some(Condition::Raw(s))
}

// ---------------------------------------------------------------------------
// Conditional block
// ---------------------------------------------------------------------------

/// A conditional block (`if`/`elif` with optional `else`) inside a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conditional<'a> {
    /// The parsed condition.
    pub condition: Condition<'a>,
    /// Fields in the then-branch.
    pub then_fields: Vec<Field<'a>>,
    /// Dependencies in the then-branch.
    pub then_deps: Vec<Dependency<'a>>,
    /// Fields in the else-branch.
    pub else_fields: Vec<Field<'a>>,
    /// Dependencies in the else-branch.
    pub else_deps: Vec<Dependency<'a>>,
    /// Nested conditionals in the then-branch.
    pub then_conditionals: Vec<Conditional<'a>>,
    /// Nested conditionals in the else-branch.
    pub else_conditionals: Vec<Conditional<'a>>,
    /// Back-reference to the CST conditional node.
    pub cst_node: NodeId,
}

// ---------------------------------------------------------------------------
// Component types
// ---------------------------------------------------------------------------

/// Shared fields across all component types (library, executable, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFields<'a> {
    /// Component name (`None` for the unnamed default library).
    pub name: Option<&'a str>,
    /// Back-reference to the CST section node.
    pub cst_node: NodeId,
    /// `import:` directives.
    pub imports: Vec<&'a str>,
    /// `build-depends` entries.
    pub build_depends: Vec<Dependency<'a>>,
    /// `other-modules` entries.
    pub other_modules: Vec<&'a str>,
    /// `hs-source-dirs` entries.
    pub hs_source_dirs: Vec<&'a str>,
    /// `default-language` value.
    pub default_language: Option<&'a str>,
    /// `default-extensions` entries.
    pub default_extensions: Vec<&'a str>,
    /// `ghc-options` entries.
    pub ghc_options: Vec<&'a str>,
    /// Fields not specifically parsed.
    pub other_fields: Vec<Field<'a>>,
    /// Conditional blocks within this component.
    pub conditionals: Vec<Conditional<'a>>,
}

/// A library component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library<'a> {
    /// Shared component fields.
    pub fields: ComponentFields<'a>,
    /// `exposed-modules` entries.
    pub exposed_modules: Vec<&'a str>,
}

/// An executable component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executable<'a> {
    /// Shared component fields.
    pub fields: ComponentFields<'a>,
    /// `main-is` value.
    pub main_is: Option<&'a str>,
}

/// A test-suite component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSuite<'a> {
    /// Shared component fields.
    pub fields: ComponentFields<'a>,
    /// `type` value (e.g. `exitcode-stdio-1.0`).
    pub test_type: Option<&'a str>,
    /// `main-is` value.
    pub main_is: Option<&'a str>,
}

/// A benchmark component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Benchmark<'a> {
    /// Shared component fields.
    pub fields: ComponentFields<'a>,
    /// `type` value (e.g. `exitcode-stdio-1.0`).
    pub bench_type: Option<&'a str>,
    /// `main-is` value.
    pub main_is: Option<&'a str>,
}

/// A `common` stanza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonStanza<'a> {
    /// The stanza name.
    pub name: &'a str,
    /// Shared component fields.
    pub fields: ComponentFields<'a>,
}

/// A `flag` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag<'a> {
    /// The flag name.
    pub name: &'a str,
    /// Description, if present.
    pub description: Option<&'a str>,
    /// Default value, if present.
    pub default: Option<bool>,
    /// Whether the flag is manual.
    pub manual: Option<bool>,
    /// All other fields.
    pub other_fields: Vec<Field<'a>>,
    /// Back-reference to the CST section node.
    pub cst_node: NodeId,
}

/// A `source-repository` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRepository<'a> {
    /// The kind (e.g. `"head"`, `"this"`).
    pub kind: Option<&'a str>,
    /// Repository type (e.g. `"git"`, `"darcs"`).
    pub repo_type: Option<&'a str>,
    /// Repository location URL.
    pub location: Option<&'a str>,
    /// Tag, if specified.
    pub tag: Option<&'a str>,
    /// Branch, if specified.
    pub branch: Option<&'a str>,
    /// Subdir, if specified.
    pub subdir: Option<&'a str>,
    /// All other fields.
    pub other_fields: Vec<Field<'a>>,
    /// Back-reference to the CST section node.
    pub cst_node: NodeId,
}

// ---------------------------------------------------------------------------
// Component enum (for uniform access)
// ---------------------------------------------------------------------------

/// A reference to any component type.
#[derive(Debug, Clone)]
pub enum Component<'a, 'b> {
    Library(&'b Library<'a>),
    Executable(&'b Executable<'a>),
    TestSuite(&'b TestSuite<'a>),
    Benchmark(&'b Benchmark<'a>),
}

impl<'a, 'b> Component<'a, 'b> {
    /// Get the shared component fields.
    pub fn fields(&self) -> &ComponentFields<'a> {
        match self {
            Component::Library(l) => &l.fields,
            Component::Executable(e) => &e.fields,
            Component::TestSuite(t) => &t.fields,
            Component::Benchmark(b) => &b.fields,
        }
    }

    /// Get the component name.
    pub fn name(&self) -> Option<&'a str> {
        self.fields().name
    }

    /// Get the CST node back-reference.
    pub fn cst_node(&self) -> NodeId {
        self.fields().cst_node
    }
}

// ---------------------------------------------------------------------------
// Top-level AST
// ---------------------------------------------------------------------------

/// The top-level AST for a parsed `.cabal` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CabalFile<'a> {
    /// Reference to the source text.
    pub source: &'a str,
    /// The `cabal-version` field.
    pub cabal_version: Option<CabalVersion<'a>>,
    /// Package name.
    pub name: Option<&'a str>,
    /// Package version.
    pub version: Option<Version>,
    /// License identifier.
    pub license: Option<&'a str>,
    /// One-line package summary.
    pub synopsis: Option<&'a str>,
    /// Longer package description.
    pub description: Option<&'a str>,
    /// Author name(s).
    pub author: Option<&'a str>,
    /// Maintainer email/name.
    pub maintainer: Option<&'a str>,
    /// Package homepage URL.
    pub homepage: Option<&'a str>,
    /// Bug tracker URL.
    pub bug_reports: Option<&'a str>,
    /// Category string.
    pub category: Option<&'a str>,
    /// Build type (`Simple`, `Configure`, `Make`, `Custom`).
    pub build_type: Option<&'a str>,
    /// `tested-with` field.
    pub tested_with: Option<&'a str>,
    /// Extra source files.
    pub extra_source_files: Vec<&'a str>,
    /// Top-level fields not specifically parsed.
    pub other_fields: Vec<Field<'a>>,
    /// `common` stanzas.
    pub common_stanzas: Vec<CommonStanza<'a>>,
    /// `flag` sections.
    pub flags: Vec<Flag<'a>>,
    /// The unnamed default library (if present).
    pub library: Option<Library<'a>>,
    /// Named internal libraries.
    pub named_libraries: Vec<Library<'a>>,
    /// Executable components.
    pub executables: Vec<Executable<'a>>,
    /// Test suite components.
    pub test_suites: Vec<TestSuite<'a>>,
    /// Benchmark components.
    pub benchmarks: Vec<Benchmark<'a>>,
    /// Source repository sections.
    pub source_repositories: Vec<SourceRepository<'a>>,
    /// Back-reference to the CST root node.
    pub cst_root: NodeId,
}

impl<'a> CabalFile<'a> {
    /// Collect all dependencies across all components, including conditional
    /// blocks.
    pub fn all_dependencies(&self) -> Vec<&Dependency<'a>> {
        let mut deps = Vec::new();

        if let Some(ref lib) = self.library {
            collect_component_deps(&lib.fields, &mut deps);
        }
        for lib in &self.named_libraries {
            collect_component_deps(&lib.fields, &mut deps);
        }
        for exe in &self.executables {
            collect_component_deps(&exe.fields, &mut deps);
        }
        for ts in &self.test_suites {
            collect_component_deps(&ts.fields, &mut deps);
        }
        for bm in &self.benchmarks {
            collect_component_deps(&bm.fields, &mut deps);
        }
        for cs in &self.common_stanzas {
            collect_component_deps(&cs.fields, &mut deps);
        }

        deps
    }

    /// Return references to all components (library, executables, test suites,
    /// benchmarks).
    pub fn all_components(&self) -> Vec<Component<'a, '_>> {
        let mut comps = Vec::new();
        if let Some(ref lib) = self.library {
            comps.push(Component::Library(lib));
        }
        for lib in &self.named_libraries {
            comps.push(Component::Library(lib));
        }
        for exe in &self.executables {
            comps.push(Component::Executable(exe));
        }
        for ts in &self.test_suites {
            comps.push(Component::TestSuite(ts));
        }
        for bm in &self.benchmarks {
            comps.push(Component::Benchmark(bm));
        }
        comps
    }

    /// Find a component by name.
    ///
    /// The unnamed library can be found by passing `"library"`.
    pub fn find_component(&self, name: &str) -> Option<Component<'a, '_>> {
        if let Some(ref lib) = self.library {
            if name == "library" || lib.fields.name == Some(name) {
                return Some(Component::Library(lib));
            }
        }
        for lib in &self.named_libraries {
            if lib.fields.name == Some(name) {
                return Some(Component::Library(lib));
            }
        }
        for exe in &self.executables {
            if exe.fields.name == Some(name) {
                return Some(Component::Executable(exe));
            }
        }
        for ts in &self.test_suites {
            if ts.fields.name == Some(name) {
                return Some(Component::TestSuite(ts));
            }
        }
        for bm in &self.benchmarks {
            if bm.fields.name == Some(name) {
                return Some(Component::Benchmark(bm));
            }
        }
        None
    }
}

/// Collect deps from a component's fields, including conditional deps.
fn collect_component_deps<'a, 'b>(
    fields: &'b ComponentFields<'a>,
    deps: &mut Vec<&'b Dependency<'a>>,
) {
    for d in &fields.build_depends {
        deps.push(d);
    }
    collect_conditional_deps(&fields.conditionals, deps);
}

fn collect_conditional_deps<'a, 'b>(
    conditionals: &'b [Conditional<'a>],
    deps: &mut Vec<&'b Dependency<'a>>,
) {
    for cond in conditionals {
        for d in &cond.then_deps {
            deps.push(d);
        }
        for d in &cond.else_deps {
            deps.push(d);
        }
        collect_conditional_deps(&cond.then_conditionals, deps);
        collect_conditional_deps(&cond.else_conditionals, deps);
    }
}

// ---------------------------------------------------------------------------
// AST derivation from CST
// ---------------------------------------------------------------------------

/// Derive a typed AST from a parsed CST.
///
/// Walks the CST root's children, matching field names to known metadata
/// fields, and section keywords to component types. Within each section,
/// fields are parsed into typed representations.
pub fn derive_ast<'a>(cst: &'a CabalCst) -> CabalFile<'a> {
    let source = cst.source.as_str();
    let mut file = CabalFile {
        source,
        cabal_version: None,
        name: None,
        version: None,
        license: None,
        synopsis: None,
        description: None,
        author: None,
        maintainer: None,
        homepage: None,
        bug_reports: None,
        category: None,
        build_type: None,
        tested_with: None,
        extra_source_files: Vec::new(),
        other_fields: Vec::new(),
        common_stanzas: Vec::new(),
        flags: Vec::new(),
        library: None,
        named_libraries: Vec::new(),
        executables: Vec::new(),
        test_suites: Vec::new(),
        benchmarks: Vec::new(),
        source_repositories: Vec::new(),
        cst_root: cst.root,
    };

    // Walk all nodes recursively. The CST parser may nest top-level sections
    // inside each other when they are at indent 0, so we cannot rely on only
    // examining direct children of root.
    collect_ast_nodes(cst, cst.root, &mut file);

    file
}

/// Recursively walk the CST node tree and collect top-level fields and
/// sections into the CabalFile. This handles the case where the CST parser
/// nests sibling sections inside each other (due to indent-based parsing
/// with all top-level sections at column 0).
fn collect_ast_nodes<'a>(cst: &'a CabalCst, node_id: NodeId, file: &mut CabalFile<'a>) {
    let node = cst.node(node_id);

    match node.kind {
        CstNodeKind::Root => {
            // Process direct children.
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                collect_ast_nodes(cst, child_id, file);
            }
        }
        CstNodeKind::Field => {
            // Only process as top-level if parent is Root.
            if cst.node(node_id).parent == Some(cst.root) {
                derive_top_level_field(cst, node_id, file);
            }
        }
        CstNodeKind::Section => {
            // Check if this is a known section type (library, executable, etc.).
            let source = cst.source.as_str();
            let is_top_level_section = if let Some(ref kw_span) = node.section_keyword {
                let kw = kw_span.slice(source).to_ascii_lowercase();
                matches!(
                    kw.as_str(),
                    "library"
                        | "executable"
                        | "test-suite"
                        | "benchmark"
                        | "common"
                        | "flag"
                        | "source-repository"
                )
            } else {
                false
            };

            if is_top_level_section {
                derive_section(cst, node_id, file);
                // Also recursively check this section's children for more
                // sections that may have been nested by the parser.
                let children: Vec<NodeId> = cst.node(node_id).children.clone();
                for child_id in children {
                    let child = cst.node(child_id);
                    if child.kind == CstNodeKind::Section {
                        collect_ast_nodes(cst, child_id, file);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Extract the full value text for a field node, including continuation
/// (ValueLine) children. The result is trimmed.
fn field_full_value(cst: &CabalCst, node_id: NodeId) -> String {
    let node = cst.node(node_id);
    let source = cst.source.as_str();

    let mut value = String::new();

    // First line value.
    if let Some(ref val_span) = node.field_value {
        value.push_str(val_span.slice(source).trim());
    }

    // Continuation lines.
    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            let line_text = child.content_span.slice(source).trim();
            if !line_text.is_empty() {
                if !value.is_empty() {
                    value.push('\n');
                }
                value.push_str(line_text);
            }
        }
    }

    value
}

/// Get the first-line value as a borrowed str reference (trimmed).
/// Returns `None` if there is no field value.
fn field_first_line_value(cst: &CabalCst, node_id: NodeId) -> Option<&str> {
    let node = cst.node(node_id);
    let source = cst.source.as_str();
    node.field_value
        .as_ref()
        .map(|span| span.slice(source).trim())
        .filter(|s| !s.is_empty())
}

/// Parse a whitespace/newline-separated list from a field value.
/// Used for `exposed-modules`, `other-modules`, `hs-source-dirs`,
/// `default-extensions`, etc.
fn parse_list_field(cst: &CabalCst, node_id: NodeId) -> Vec<&str> {
    let node = cst.node(node_id);
    let source = cst.source.as_str();
    let mut items = Vec::new();

    // First line.
    if let Some(ref val_span) = node.field_value {
        let text = val_span.slice(source).trim();
        for item in split_list_items(text) {
            if !item.is_empty() {
                items.push(item);
            }
        }
    }

    // Continuation lines.
    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            let text = child.content_span.slice(source).trim();
            for item in split_list_items(text) {
                if !item.is_empty() {
                    items.push(item);
                }
            }
        }
    }

    items
}

/// Split a line into list items, handling commas as separators and stripping
/// leading/trailing commas.
fn split_list_items(text: &str) -> Vec<&str> {
    let mut items = Vec::new();
    if text.contains(',') {
        for part in text.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                items.push(trimmed);
            }
        }
    } else {
        // Space-separated.
        for part in text.split_whitespace() {
            items.push(part);
        }
    }
    items
}

/// Parse `ghc-options` value: space-separated tokens, possibly multi-line.
fn parse_ghc_options(cst: &CabalCst, node_id: NodeId) -> Vec<&str> {
    let node = cst.node(node_id);
    let source = cst.source.as_str();
    let mut opts = Vec::new();

    if let Some(ref val_span) = node.field_value {
        for opt in val_span.slice(source).split_whitespace() {
            opts.push(opt);
        }
    }

    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            for opt in child.content_span.slice(source).split_whitespace() {
                opts.push(opt);
            }
        }
    }

    opts
}

/// Parse dependencies from a `build-depends` field node.
fn parse_build_depends<'a>(cst: &'a CabalCst, node_id: NodeId) -> Vec<Dependency<'a>> {
    let node = cst.node(node_id);
    let source = cst.source.as_str();
    let mut deps = Vec::new();

    // First line value.
    if let Some(ref val_span) = node.field_value {
        let text = val_span.slice(source).trim();
        deps.extend(parse_dependencies_from_text(text, node_id));
    }

    // Continuation lines.
    for &child_id in &node.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::ValueLine {
            let text = child.content_span.slice(source).trim();
            if !text.is_empty() {
                // Use the child's NodeId so back-references point to the
                // specific ValueLine.
                deps.extend(parse_dependencies_from_text(text, child_id));
            }
        }
    }

    deps
}

/// Derive a top-level field into the CabalFile metadata.
fn derive_top_level_field<'a>(cst: &'a CabalCst, node_id: NodeId, file: &mut CabalFile<'a>) {
    let node = cst.node(node_id);
    let source = cst.source.as_str();

    let raw_name = match node.field_name {
        Some(ref span) => span.slice(source),
        None => return,
    };
    let canon = canonicalize_field_name(raw_name);

    match canon.as_str() {
        "cabal-version" => {
            let raw = field_first_line_value(cst, node_id).unwrap_or("");
            // Strip leading `>=` that some old files use.
            let version_str = raw.strip_prefix(">=").unwrap_or(raw).trim();
            file.cabal_version = Some(CabalVersion {
                raw,
                version: Version::parse(version_str),
                cst_node: node_id,
            });
        }
        "name" => {
            file.name = field_first_line_value(cst, node_id);
        }
        "version" => {
            let raw = field_first_line_value(cst, node_id).unwrap_or("");
            file.version = Version::parse(raw);
        }
        "license" => {
            file.license = field_first_line_value(cst, node_id);
        }
        "synopsis" => {
            file.synopsis = field_first_line_value(cst, node_id);
        }
        "description" => {
            // Description can be multi-line; we store a reference to the first
            // line and let callers use `field_full_value` if they need all of
            // it. For the AST we just capture the first line.
            file.description = field_first_line_value(cst, node_id);
        }
        "author" => {
            file.author = field_first_line_value(cst, node_id);
        }
        "maintainer" => {
            file.maintainer = field_first_line_value(cst, node_id);
        }
        "homepage" => {
            file.homepage = field_first_line_value(cst, node_id);
        }
        "bug-reports" => {
            file.bug_reports = field_first_line_value(cst, node_id);
        }
        "category" => {
            file.category = field_first_line_value(cst, node_id);
        }
        "build-type" => {
            file.build_type = field_first_line_value(cst, node_id);
        }
        "tested-with" => {
            file.tested_with = field_first_line_value(cst, node_id);
        }
        "extra-source-files" | "extra-doc-files" => {
            file.extra_source_files
                .extend(parse_list_field(cst, node_id));
        }
        _ => {
            let value = field_full_value(cst, node_id);
            file.other_fields.push(Field {
                name: canon,
                raw_name,
                value,
                cst_node: node_id,
            });
        }
    }
}

/// Derive a section (library, executable, etc.) into the CabalFile.
fn derive_section<'a>(cst: &'a CabalCst, node_id: NodeId, file: &mut CabalFile<'a>) {
    let node = cst.node(node_id);
    let source = cst.source.as_str();

    let keyword = match node.section_keyword {
        Some(ref span) => span.slice(source),
        None => return,
    };
    let section_arg = node.section_arg.map(|span| span.slice(source));
    let keyword_lower = keyword.to_ascii_lowercase();

    match keyword_lower.as_str() {
        "library" => {
            let lib = derive_library(cst, node_id, section_arg);
            if section_arg.is_some() {
                file.named_libraries.push(lib);
            } else {
                file.library = Some(lib);
            }
        }
        "executable" => {
            let exe = derive_executable(cst, node_id, section_arg);
            file.executables.push(exe);
        }
        "test-suite" => {
            let ts = derive_test_suite(cst, node_id, section_arg);
            file.test_suites.push(ts);
        }
        "benchmark" => {
            let bm = derive_benchmark(cst, node_id, section_arg);
            file.benchmarks.push(bm);
        }
        "common" => {
            if let Some(name) = section_arg {
                let cs = derive_common_stanza(cst, node_id, name);
                file.common_stanzas.push(cs);
            }
        }
        "flag" => {
            if let Some(name) = section_arg {
                let flag = derive_flag(cst, node_id, name);
                file.flags.push(flag);
            }
        }
        "source-repository" => {
            let sr = derive_source_repository(cst, node_id, section_arg);
            file.source_repositories.push(sr);
        }
        _ => {
            // Unknown section type — ignore for now.
        }
    }
}

/// Create default empty `ComponentFields`.
fn empty_component_fields<'a>(name: Option<&'a str>, cst_node: NodeId) -> ComponentFields<'a> {
    ComponentFields {
        name,
        cst_node,
        imports: Vec::new(),
        build_depends: Vec::new(),
        other_modules: Vec::new(),
        hs_source_dirs: Vec::new(),
        default_language: None,
        default_extensions: Vec::new(),
        ghc_options: Vec::new(),
        other_fields: Vec::new(),
        conditionals: Vec::new(),
    }
}

/// Populate `ComponentFields` from the children of a section node.
fn populate_component_fields<'a>(
    cst: &'a CabalCst,
    section_id: NodeId,
    fields: &mut ComponentFields<'a>,
) {
    let section = cst.node(section_id);
    let source = cst.source.as_str();

    for &child_id in &section.children {
        let child = cst.node(child_id);
        match child.kind {
            CstNodeKind::Field => {
                let raw_name = match child.field_name {
                    Some(ref span) => span.slice(source),
                    None => continue,
                };
                let canon = canonicalize_field_name(raw_name);

                match canon.as_str() {
                    "build-depends" => {
                        fields
                            .build_depends
                            .extend(parse_build_depends(cst, child_id));
                    }
                    "exposed-modules" => {
                        // Handled by caller if Library.
                        // We still parse here and caller picks it up.
                    }
                    "other-modules" => {
                        fields.other_modules.extend(parse_list_field(cst, child_id));
                    }
                    "hs-source-dirs" => {
                        fields
                            .hs_source_dirs
                            .extend(parse_list_field(cst, child_id));
                    }
                    "default-language" => {
                        fields.default_language = field_first_line_value(cst, child_id);
                    }
                    "default-extensions" | "extensions" => {
                        fields
                            .default_extensions
                            .extend(parse_list_field(cst, child_id));
                    }
                    "ghc-options" => {
                        fields.ghc_options.extend(parse_ghc_options(cst, child_id));
                    }
                    _ => {
                        let value = field_full_value(cst, child_id);
                        fields.other_fields.push(Field {
                            name: canon,
                            raw_name,
                            value,
                            cst_node: child_id,
                        });
                    }
                }
            }
            CstNodeKind::Import => {
                if let Some(ref val_span) = child.field_value {
                    let val = val_span.slice(source).trim();
                    if !val.is_empty() {
                        // imports can be comma-separated
                        for item in val.split(',') {
                            let item = item.trim();
                            if !item.is_empty() {
                                fields.imports.push(item);
                            }
                        }
                    }
                }
            }
            CstNodeKind::Conditional => {
                let cond = derive_conditional(cst, child_id);
                fields.conditionals.push(cond);
            }
            // Comments, blank lines, value lines — skip for AST.
            _ => {}
        }
    }
}

/// Derive a conditional block from a CST Conditional node.
fn derive_conditional<'a>(cst: &'a CabalCst, node_id: NodeId) -> Conditional<'a> {
    let node = cst.node(node_id);
    let source = cst.source.as_str();

    // Parse condition expression.
    let condition = match node.condition_expr {
        Some(ref span) => parse_condition(span.slice(source)),
        None => Condition::Raw(""),
    };

    let mut cond = Conditional {
        condition,
        then_fields: Vec::new(),
        then_deps: Vec::new(),
        else_fields: Vec::new(),
        else_deps: Vec::new(),
        then_conditionals: Vec::new(),
        else_conditionals: Vec::new(),
        cst_node: node_id,
    };

    // Process children: then-block items, then the ElseBlock.
    for &child_id in &node.children {
        let child = cst.node(child_id);
        match child.kind {
            CstNodeKind::Field => {
                let raw_name = match child.field_name {
                    Some(ref span) => span.slice(source),
                    None => continue,
                };
                let canon = canonicalize_field_name(raw_name);

                if canon == "build-depends" {
                    cond.then_deps.extend(parse_build_depends(cst, child_id));
                } else {
                    let value = field_full_value(cst, child_id);
                    cond.then_fields.push(Field {
                        name: canon,
                        raw_name,
                        value,
                        cst_node: child_id,
                    });
                }
            }
            CstNodeKind::Conditional => {
                cond.then_conditionals
                    .push(derive_conditional(cst, child_id));
            }
            CstNodeKind::ElseBlock => {
                // Process else block children.
                for &else_child_id in &child.children {
                    let else_child = cst.node(else_child_id);
                    match else_child.kind {
                        CstNodeKind::Field => {
                            let raw_name = match else_child.field_name {
                                Some(ref span) => span.slice(source),
                                None => continue,
                            };
                            let canon = canonicalize_field_name(raw_name);

                            if canon == "build-depends" {
                                cond.else_deps
                                    .extend(parse_build_depends(cst, else_child_id));
                            } else {
                                let value = field_full_value(cst, else_child_id);
                                cond.else_fields.push(Field {
                                    name: canon,
                                    raw_name,
                                    value,
                                    cst_node: else_child_id,
                                });
                            }
                        }
                        CstNodeKind::Conditional => {
                            cond.else_conditionals
                                .push(derive_conditional(cst, else_child_id));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    cond
}

/// Derive a Library from a CST section node.
fn derive_library<'a>(cst: &'a CabalCst, node_id: NodeId, name: Option<&'a str>) -> Library<'a> {
    let mut fields = empty_component_fields(name, node_id);
    populate_component_fields(cst, node_id, &mut fields);

    // Extract exposed-modules from section children (since populate_component_fields
    // skips it for the generic path).
    let exposed_modules = extract_exposed_modules(cst, node_id);

    Library {
        fields,
        exposed_modules,
    }
}

/// Extract `exposed-modules` from a section's children.
fn extract_exposed_modules(cst: &CabalCst, section_id: NodeId) -> Vec<&str> {
    let section = cst.node(section_id);
    let source = cst.source.as_str();
    let mut modules = Vec::new();

    for &child_id in &section.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field {
            if let Some(ref name_span) = child.field_name {
                let canon = canonicalize_field_name(name_span.slice(source));
                if canon == "exposed-modules" {
                    modules.extend(parse_list_field(cst, child_id));
                }
            }
        }
    }

    modules
}

/// Derive an Executable from a CST section node.
fn derive_executable<'a>(
    cst: &'a CabalCst,
    node_id: NodeId,
    name: Option<&'a str>,
) -> Executable<'a> {
    let main_is = find_field_value_in_section(cst, node_id, "main-is");

    let mut fields = empty_component_fields(name, node_id);
    populate_component_fields(cst, node_id, &mut fields);
    remove_field_by_name(&mut fields.other_fields, "main-is");

    Executable { fields, main_is }
}

/// Derive a TestSuite from a CST section node.
fn derive_test_suite<'a>(
    cst: &'a CabalCst,
    node_id: NodeId,
    name: Option<&'a str>,
) -> TestSuite<'a> {
    let test_type = find_field_value_in_section(cst, node_id, "type");
    let main_is = find_field_value_in_section(cst, node_id, "main-is");

    let mut fields = empty_component_fields(name, node_id);
    populate_component_fields(cst, node_id, &mut fields);
    remove_field_by_name(&mut fields.other_fields, "type");
    remove_field_by_name(&mut fields.other_fields, "main-is");

    TestSuite {
        fields,
        test_type,
        main_is,
    }
}

/// Derive a Benchmark from a CST section node.
fn derive_benchmark<'a>(
    cst: &'a CabalCst,
    node_id: NodeId,
    name: Option<&'a str>,
) -> Benchmark<'a> {
    let bench_type = find_field_value_in_section(cst, node_id, "type");
    let main_is = find_field_value_in_section(cst, node_id, "main-is");

    let mut fields = empty_component_fields(name, node_id);
    populate_component_fields(cst, node_id, &mut fields);
    remove_field_by_name(&mut fields.other_fields, "type");
    remove_field_by_name(&mut fields.other_fields, "main-is");

    Benchmark {
        fields,
        bench_type,
        main_is,
    }
}

/// Derive a CommonStanza from a CST section node.
fn derive_common_stanza<'a>(cst: &'a CabalCst, node_id: NodeId, name: &'a str) -> CommonStanza<'a> {
    let mut fields = empty_component_fields(Some(name), node_id);
    populate_component_fields(cst, node_id, &mut fields);

    CommonStanza { name, fields }
}

/// Derive a Flag from a CST section node.
fn derive_flag<'a>(cst: &'a CabalCst, node_id: NodeId, name: &'a str) -> Flag<'a> {
    let section = cst.node(node_id);
    let source = cst.source.as_str();

    let mut description = None;
    let mut default = None;
    let mut manual = None;
    let mut other_fields = Vec::new();

    for &child_id in &section.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field {
            let raw_name = match child.field_name {
                Some(ref span) => span.slice(source),
                None => continue,
            };
            let canon = canonicalize_field_name(raw_name);

            match canon.as_str() {
                "description" => {
                    description = field_first_line_value(cst, child_id);
                }
                "default" => {
                    if let Some(val) = field_first_line_value(cst, child_id) {
                        let lower = val.to_ascii_lowercase();
                        default = Some(lower == "true");
                    }
                }
                "manual" => {
                    if let Some(val) = field_first_line_value(cst, child_id) {
                        let lower = val.to_ascii_lowercase();
                        manual = Some(lower == "true");
                    }
                }
                _ => {
                    let value = field_full_value(cst, child_id);
                    other_fields.push(Field {
                        name: canon,
                        raw_name,
                        value,
                        cst_node: child_id,
                    });
                }
            }
        }
    }

    Flag {
        name,
        description,
        default,
        manual,
        other_fields,
        cst_node: node_id,
    }
}

/// Derive a SourceRepository from a CST section node.
fn derive_source_repository<'a>(
    cst: &'a CabalCst,
    node_id: NodeId,
    kind: Option<&'a str>,
) -> SourceRepository<'a> {
    let section = cst.node(node_id);
    let source = cst.source.as_str();

    let mut repo_type = None;
    let mut location = None;
    let mut tag = None;
    let mut branch = None;
    let mut subdir = None;
    let mut other_fields = Vec::new();

    for &child_id in &section.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field {
            let raw_name = match child.field_name {
                Some(ref span) => span.slice(source),
                None => continue,
            };
            let canon = canonicalize_field_name(raw_name);

            match canon.as_str() {
                "type" => {
                    repo_type = field_first_line_value(cst, child_id);
                }
                "location" => {
                    location = field_first_line_value(cst, child_id);
                }
                "tag" => {
                    tag = field_first_line_value(cst, child_id);
                }
                "branch" => {
                    branch = field_first_line_value(cst, child_id);
                }
                "subdir" => {
                    subdir = field_first_line_value(cst, child_id);
                }
                _ => {
                    let value = field_full_value(cst, child_id);
                    other_fields.push(Field {
                        name: canon,
                        raw_name,
                        value,
                        cst_node: child_id,
                    });
                }
            }
        }
    }

    SourceRepository {
        kind,
        repo_type,
        location,
        tag,
        branch,
        subdir,
        other_fields,
        cst_node: node_id,
    }
}

/// Remove a field by canonicalized name from `other_fields`.
fn remove_field_by_name(fields: &mut Vec<Field<'_>>, canonical_name: &str) {
    fields.retain(|f| f.name != canonical_name);
}

/// Look up a field by canonicalized name in a section's children and return
/// its first-line value.
fn find_field_value_in_section<'a>(
    cst: &'a CabalCst,
    section_id: NodeId,
    target_canon: &str,
) -> Option<&'a str> {
    let section = cst.node(section_id);
    let source = cst.source.as_str();

    for &child_id in &section.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Field {
            if let Some(ref name_span) = child.field_name {
                let canon = canonicalize_field_name(name_span.slice(source));
                if canon == target_canon {
                    return field_first_line_value(cst, child_id);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse source and return the ParseResult. Callers derive the AST from it.
    fn do_parse(source: &str) -> crate::parse::ParseResult {
        crate::parse::parse(source)
    }

    // -- Version parsing tests ------------------------------------------------

    #[test]
    fn version_parse_simple() {
        let v = Version::parse("0.1.0.0").unwrap();
        assert_eq!(v.components, vec![0, 1, 0, 0]);
    }

    #[test]
    fn version_parse_two_components() {
        let v = Version::parse("4.14").unwrap();
        assert_eq!(v.components, vec![4, 14]);
    }

    #[test]
    fn version_parse_single() {
        let v = Version::parse("5").unwrap();
        assert_eq!(v.components, vec![5]);
    }

    #[test]
    fn version_parse_empty() {
        assert!(Version::parse("").is_none());
    }

    #[test]
    fn version_parse_invalid() {
        assert!(Version::parse("abc").is_none());
        assert!(Version::parse("1.2.abc").is_none());
    }

    #[test]
    fn version_display() {
        let v = Version {
            components: vec![1, 2, 3, 0],
        };
        assert_eq!(v.to_string(), "1.2.3.0");
    }

    // -- Version range parsing tests ------------------------------------------

    #[test]
    fn version_range_gte() {
        let vr = parse_version_range(">=4.14").unwrap();
        assert_eq!(
            vr,
            VersionRange::Gte(Version {
                components: vec![4, 14]
            })
        );
    }

    #[test]
    fn version_range_lt() {
        let vr = parse_version_range("<5").unwrap();
        assert_eq!(
            vr,
            VersionRange::Lt(Version {
                components: vec![5]
            })
        );
    }

    #[test]
    fn version_range_major_bound() {
        let vr = parse_version_range("^>=2.2").unwrap();
        assert_eq!(
            vr,
            VersionRange::MajorBound(Version {
                components: vec![2, 2]
            })
        );
    }

    #[test]
    fn version_range_eq() {
        let vr = parse_version_range("==1.0").unwrap();
        assert_eq!(
            vr,
            VersionRange::Eq(Version {
                components: vec![1, 0]
            })
        );
    }

    #[test]
    fn version_range_and() {
        let vr = parse_version_range(">=4.14 && <5").unwrap();
        assert_eq!(
            vr,
            VersionRange::And(
                Box::new(VersionRange::Gte(Version {
                    components: vec![4, 14]
                })),
                Box::new(VersionRange::Lt(Version {
                    components: vec![5]
                })),
            )
        );
    }

    #[test]
    fn version_range_or() {
        let vr = parse_version_range(">=2.0 || ==1.9").unwrap();
        assert_eq!(
            vr,
            VersionRange::Or(
                Box::new(VersionRange::Gte(Version {
                    components: vec![2, 0]
                })),
                Box::new(VersionRange::Eq(Version {
                    components: vec![1, 9]
                })),
            )
        );
    }

    #[test]
    fn version_range_complex_and() {
        let vr = parse_version_range(">=2.0 && <2.2").unwrap();
        assert_eq!(
            vr,
            VersionRange::And(
                Box::new(VersionRange::Gte(Version {
                    components: vec![2, 0]
                })),
                Box::new(VersionRange::Lt(Version {
                    components: vec![2, 2]
                })),
            )
        );
    }

    #[test]
    fn version_range_empty() {
        assert!(parse_version_range("").is_none());
    }

    // -- Canonicalize field name tests ----------------------------------------

    #[test]
    fn canonicalize_mixed_case() {
        assert_eq!(canonicalize_field_name("Build-Depends"), "build-depends");
    }

    #[test]
    fn canonicalize_underscore() {
        assert_eq!(canonicalize_field_name("build_depends"), "build-depends");
    }

    #[test]
    fn canonicalize_already_canonical() {
        assert_eq!(canonicalize_field_name("build-depends"), "build-depends");
    }

    // -- Dependency parsing tests ---------------------------------------------

    #[test]
    fn parse_dep_no_version() {
        let dep = parse_single_dependency("base", NodeId(0)).unwrap();
        assert_eq!(dep.package, "base");
        assert!(dep.version_range.is_none());
    }

    #[test]
    fn parse_dep_with_version() {
        let dep = parse_single_dependency("aeson ^>=2.2", NodeId(0)).unwrap();
        assert_eq!(dep.package, "aeson");
        assert_eq!(
            dep.version_range,
            Some(VersionRange::MajorBound(Version {
                components: vec![2, 2]
            }))
        );
    }

    #[test]
    fn parse_dep_with_range() {
        let dep = parse_single_dependency("base >=4.14 && <5", NodeId(0)).unwrap();
        assert_eq!(dep.package, "base");
        assert_eq!(
            dep.version_range,
            Some(VersionRange::And(
                Box::new(VersionRange::Gte(Version {
                    components: vec![4, 14]
                })),
                Box::new(VersionRange::Lt(Version {
                    components: vec![5]
                })),
            ))
        );
    }

    #[test]
    fn parse_deps_comma_separated() {
        let deps = parse_dependencies_from_text("base >=4.14, text >=2.0, aeson ^>=2.2", NodeId(0));
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].package, "base");
        assert_eq!(deps[1].package, "text");
        assert_eq!(deps[2].package, "aeson");
    }

    #[test]
    fn parse_deps_empty() {
        let deps = parse_dependencies_from_text("", NodeId(0));
        assert!(deps.is_empty());
    }

    // -- Condition parsing tests ----------------------------------------------

    #[test]
    fn parse_condition_flag() {
        let c = parse_condition("flag(dev)");
        assert_eq!(c, Condition::Flag("dev"));
    }

    #[test]
    fn parse_condition_os() {
        let c = parse_condition("os(windows)");
        assert_eq!(c, Condition::OS("windows"));
    }

    #[test]
    fn parse_condition_arch() {
        let c = parse_condition("arch(x86_64)");
        assert_eq!(c, Condition::Arch("x86_64"));
    }

    #[test]
    fn parse_condition_impl() {
        let c = parse_condition("impl(ghc >= 9.6)");
        assert_eq!(
            c,
            Condition::Impl(
                "ghc",
                Some(VersionRange::Gte(Version {
                    components: vec![9, 6]
                }))
            )
        );
    }

    #[test]
    fn parse_condition_not() {
        let c = parse_condition("!os(windows)");
        assert_eq!(c, Condition::Not(Box::new(Condition::OS("windows"))));
    }

    #[test]
    fn parse_condition_and() {
        let c = parse_condition("flag(dev) && !os(windows)");
        assert_eq!(
            c,
            Condition::And(
                Box::new(Condition::Flag("dev")),
                Box::new(Condition::Not(Box::new(Condition::OS("windows")))),
            )
        );
    }

    #[test]
    fn parse_condition_or() {
        let c = parse_condition("flag(a) || flag(b)");
        assert_eq!(
            c,
            Condition::Or(
                Box::new(Condition::Flag("a")),
                Box::new(Condition::Flag("b")),
            )
        );
    }

    #[test]
    fn parse_condition_empty() {
        let c = parse_condition("");
        assert_eq!(c, Condition::Raw(""));
    }

    // -- Full AST derivation tests --------------------------------------------

    #[test]
    fn derive_minimal_file() {
        let src = "cabal-version: 3.0\nname: my-pkg\nversion: 0.1.0.0\n";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.name, Some("my-pkg"));
        assert_eq!(
            ast.version,
            Some(Version {
                components: vec![0, 1, 0, 0]
            })
        );
        assert!(ast.cabal_version.is_some());
        let cv = ast.cabal_version.as_ref().unwrap();
        assert_eq!(cv.raw, "3.0");
        assert_eq!(
            cv.version,
            Some(Version {
                components: vec![3, 0]
            })
        );
    }

    #[test]
    fn derive_with_library() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  exposed-modules:
    Foo
    Bar
  build-depends:
    base >=4.14
  default-language: GHC2021
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert!(ast.library.is_some());
        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.exposed_modules, vec!["Foo", "Bar"]);
        assert_eq!(lib.fields.build_depends.len(), 1);
        assert_eq!(lib.fields.build_depends[0].package, "base");
        assert_eq!(lib.fields.default_language, Some("GHC2021"));
    }

    #[test]
    fn derive_with_executable() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

executable my-exe
  main-is: Main.hs
  build-depends: base
  hs-source-dirs: app
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.executables.len(), 1);
        let exe = &ast.executables[0];
        assert_eq!(exe.fields.name, Some("my-exe"));
        assert_eq!(exe.main_is, Some("Main.hs"));
        assert_eq!(exe.fields.build_depends.len(), 1);
        assert_eq!(exe.fields.hs_source_dirs, vec!["app"]);
    }

    #[test]
    fn derive_with_test_suite() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

test-suite my-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, tasty
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.test_suites.len(), 1);
        let ts = &ast.test_suites[0];
        assert_eq!(ts.fields.name, Some("my-tests"));
        assert_eq!(ts.test_type, Some("exitcode-stdio-1.0"));
        assert_eq!(ts.main_is, Some("Main.hs"));
        assert_eq!(ts.fields.build_depends.len(), 2);
    }

    #[test]
    fn derive_with_common_stanza() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

common warnings
  ghc-options: -Wall -Wcompat

library
  import: warnings
  exposed-modules: Foo
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.common_stanzas.len(), 1);
        assert_eq!(ast.common_stanzas[0].name, "warnings");
        assert_eq!(
            ast.common_stanzas[0].fields.ghc_options,
            vec!["-Wall", "-Wcompat"]
        );

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.fields.imports, vec!["warnings"]);
    }

    #[test]
    fn derive_with_flag() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

flag dev
  description: Development mode
  default: False
  manual: True
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.flags.len(), 1);
        let flag = &ast.flags[0];
        assert_eq!(flag.name, "dev");
        assert_eq!(flag.description, Some("Development mode"));
        assert_eq!(flag.default, Some(false));
        assert_eq!(flag.manual, Some(true));
    }

    #[test]
    fn derive_with_source_repository() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

source-repository head
  type: git
  location: https://github.com/example/my-pkg
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.source_repositories.len(), 1);
        let sr = &ast.source_repositories[0];
        assert_eq!(sr.kind, Some("head"));
        assert_eq!(sr.repo_type, Some("git"));
        assert_eq!(sr.location, Some("https://github.com/example/my-pkg"));
    }

    #[test]
    fn derive_conditional() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends: base
  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.fields.conditionals.len(), 1);
        let cond = &lib.fields.conditionals[0];
        assert_eq!(cond.condition, Condition::Flag("dev"));
        assert_eq!(cond.then_fields.len(), 1);
        assert_eq!(cond.then_fields[0].name, "ghc-options");
        assert_eq!(cond.then_fields[0].value, "-O0");
        assert_eq!(cond.else_fields.len(), 1);
        assert_eq!(cond.else_fields[0].name, "ghc-options");
        assert_eq!(cond.else_fields[0].value, "-O2");
    }

    #[test]
    fn derive_all_dependencies() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends: base, text

executable my-exe
  build-depends: base, my-pkg
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let all_deps = ast.all_dependencies();
        assert_eq!(all_deps.len(), 4);
        let names: Vec<&str> = all_deps.iter().map(|d| d.package).collect();
        assert!(names.contains(&"base"));
        assert!(names.contains(&"text"));
        assert!(names.contains(&"my-pkg"));
    }

    #[test]
    fn derive_all_components() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  exposed-modules: Foo

executable my-exe
  main-is: Main.hs

test-suite my-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs

benchmark my-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let comps = ast.all_components();
        assert_eq!(comps.len(), 4);
    }

    #[test]
    fn derive_find_component() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  exposed-modules: Foo

executable my-exe
  main-is: Main.hs
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert!(ast.find_component("library").is_some());
        assert!(ast.find_component("my-exe").is_some());
        assert!(ast.find_component("nonexistent").is_none());
    }

    #[test]
    fn derive_cst_node_back_references_valid() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends: base >=4.14
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        // The CST root back-reference should be valid.
        assert_eq!(ast.cst_root, result.cst.root);

        // Library's cst_node should be a valid Section node.
        let lib = ast.library.as_ref().unwrap();
        let node = result.cst.node(lib.fields.cst_node);
        assert_eq!(node.kind, CstNodeKind::Section);

        // Dependency's cst_node should be valid.
        assert!(!lib.fields.build_depends.is_empty());
        let dep_node_id = lib.fields.build_depends[0].cst_node;
        assert!(dep_node_id.0 < result.cst.node_count());
    }

    #[test]
    fn derive_deps_leading_comma_style() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.fields.build_depends.len(), 3);
        assert_eq!(lib.fields.build_depends[0].package, "base");
        assert_eq!(lib.fields.build_depends[1].package, "text");
        assert_eq!(lib.fields.build_depends[2].package, "aeson");
    }

    #[test]
    fn derive_deps_trailing_comma_style() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.fields.build_depends.len(), 3);
        assert_eq!(lib.fields.build_depends[0].package, "base");
        assert_eq!(lib.fields.build_depends[1].package, "text");
        assert_eq!(lib.fields.build_depends[2].package, "aeson");
    }

    #[test]
    fn derive_deps_single_line() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.fields.build_depends.len(), 3);
    }

    #[test]
    fn derive_default_extensions() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  default-extensions:
    OverloadedStrings
    DerivingStrategies
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        assert_eq!(
            lib.fields.default_extensions,
            vec!["OverloadedStrings", "DerivingStrategies"]
        );
    }

    #[test]
    fn derive_metadata_fields() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0
license: MIT
synopsis: A test package
author: Test Author
maintainer: test@example.com
homepage: https://example.com
bug-reports: https://example.com/issues
category: Development
build-type: Simple
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.license, Some("MIT"));
        assert_eq!(ast.synopsis, Some("A test package"));
        assert_eq!(ast.author, Some("Test Author"));
        assert_eq!(ast.maintainer, Some("test@example.com"));
        assert_eq!(ast.homepage, Some("https://example.com"));
        assert_eq!(ast.bug_reports, Some("https://example.com/issues"));
        assert_eq!(ast.category, Some("Development"));
        assert_eq!(ast.build_type, Some("Simple"));
    }

    #[test]
    fn derive_conditional_deps() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  build-depends: base
  if os(windows)
    build-depends: Win32
  else
    build-depends: unix
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        let all_deps = ast.all_dependencies();
        let names: Vec<&str> = all_deps.iter().map(|d| d.package).collect();
        assert!(names.contains(&"base"));
        assert!(names.contains(&"Win32"));
        assert!(names.contains(&"unix"));
        assert_eq!(all_deps.len(), 3);
    }

    // -- Boolean literal condition tests ----------------------------------------

    #[test]
    fn parse_condition_true() {
        assert_eq!(parse_condition("true"), Condition::Lit(true));
    }

    #[test]
    fn parse_condition_false() {
        assert_eq!(parse_condition("false"), Condition::Lit(false));
    }

    #[test]
    fn parse_condition_true_case_insensitive() {
        assert_eq!(parse_condition("True"), Condition::Lit(true));
        assert_eq!(parse_condition("FALSE"), Condition::Lit(false));
    }

    // -- Wildcard version range tests -------------------------------------------

    #[test]
    fn version_range_wildcard() {
        let r = parse_version_range("==1.2.*").unwrap();
        match r {
            VersionRange::And(a, b) => {
                assert_eq!(
                    *a,
                    VersionRange::Gte(Version {
                        components: vec![1, 2]
                    })
                );
                assert_eq!(
                    *b,
                    VersionRange::Lt(Version {
                        components: vec![1, 3]
                    })
                );
            }
            _ => panic!("Expected And range, got {:?}", r),
        }
    }

    // -- -any and -none version range tests -------------------------------------

    #[test]
    fn version_range_any_keyword() {
        assert_eq!(parse_version_range("-any").unwrap(), VersionRange::Any);
    }

    #[test]
    fn version_range_none_keyword() {
        assert_eq!(
            parse_version_range("-none").unwrap(),
            VersionRange::NoVersion
        );
    }

    // -- Set notation version range tests ---------------------------------------

    #[test]
    fn version_range_set_major_bound() {
        let r = parse_version_range("^>= { 2.6, 2.7, 2.8 }").unwrap();
        match r {
            VersionRange::Or(_, _) => {} // just verify it parses as Or
            _ => panic!("Expected Or range for set notation, got {:?}", r),
        }
    }

    #[test]
    fn version_range_set_eq() {
        let r = parse_version_range("== { 1.0, 2.0 }").unwrap();
        match r {
            VersionRange::Or(_, _) => {}
            _ => panic!("Expected Or range for set notation, got {:?}", r),
        }
    }

    // -- Display tests for new variants -----------------------------------------

    #[test]
    fn version_range_display_any() {
        assert_eq!(VersionRange::Any.to_string(), "-any");
    }

    #[test]
    fn version_range_display_none() {
        assert_eq!(VersionRange::NoVersion.to_string(), "-none");
    }

    #[test]
    fn derive_benchmark() {
        let src = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

benchmark my-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, criterion
  hs-source-dirs: bench
";
        let result = do_parse(src);
        let ast = derive_ast(&result.cst);

        assert_eq!(ast.benchmarks.len(), 1);
        let bm = &ast.benchmarks[0];
        assert_eq!(bm.fields.name, Some("my-bench"));
        assert_eq!(bm.bench_type, Some("exitcode-stdio-1.0"));
        assert_eq!(bm.main_is, Some("Main.hs"));
        assert_eq!(bm.fields.build_depends.len(), 2);
    }
}
