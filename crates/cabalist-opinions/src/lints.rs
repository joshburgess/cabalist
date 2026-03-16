//! Opinionated lints for `.cabal` files.
//!
//! Each lint is a function that inspects the parsed AST and returns structured
//! diagnostics. Lints are individually identifiable by a unique string ID so
//! users can disable specific lints in `cabalist.toml`.

use cabalist_parser::ast::{CabalFile, ComponentFields, Condition, Conditional, VersionRange};
use cabalist_parser::diagnostic::Severity;
use cabalist_parser::span::Span;

// ---------------------------------------------------------------------------
// Lint result type
// ---------------------------------------------------------------------------

/// A lint finding with a unique ID, severity, message, source span, and
/// optional suggestion.
#[derive(Debug, Clone)]
pub struct Lint {
    /// Unique lint identifier (e.g. `"missing-upper-bound"`).
    pub id: &'static str,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable message describing the issue.
    pub message: String,
    /// Source span pointing to the relevant location.
    pub span: Span,
    /// Optional suggested fix.
    pub suggestion: Option<String>,
}

// ---------------------------------------------------------------------------
// Lint configuration
// ---------------------------------------------------------------------------

/// Configuration for which lints are enabled/disabled and severity overrides.
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    /// Lint IDs to disable entirely.
    pub disabled: Vec<String>,
    /// Lint IDs to promote to error severity.
    pub errors: Vec<String>,
}

impl LintConfig {
    /// Returns `true` if the given lint ID is not in the disabled list.
    pub fn is_enabled(&self, lint_id: &str) -> bool {
        !self.disabled.iter().any(|d| d == lint_id)
    }

    /// Returns the effective severity for a lint: if the lint ID is in the
    /// `errors` list, returns `Error`; otherwise returns the `default`.
    pub fn effective_severity(&self, lint_id: &str, default: Severity) -> Severity {
        if self.errors.iter().any(|e| e == lint_id) {
            Severity::Error
        } else {
            default
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Run all enabled lints on a parsed cabal file.
///
/// Returns a vector of lint findings sorted by source position.
pub fn run_lints(file: &CabalFile<'_>, config: &LintConfig) -> Vec<Lint> {
    let mut lints = Vec::new();

    let all_linters: &[fn(&CabalFile<'_>, &LintConfig, &mut Vec<Lint>)] = &[
        lint_missing_upper_bound,
        lint_missing_lower_bound,
        lint_wide_any_version,
        lint_missing_synopsis,
        lint_missing_description,
        lint_missing_source_repo,
        lint_missing_bug_reports,
        lint_no_common_stanza,
        lint_ghc_options_werror,
        lint_missing_default_language,
        lint_exposed_no_modules,
        lint_cabal_version_low,
        lint_duplicate_dep,
        lint_unused_flag,
        lint_stale_tested_with,
    ];

    for linter in all_linters {
        linter(file, config, &mut lints);
    }

    lints.sort_by_key(|l| l.span.start);
    lints
}

// ---------------------------------------------------------------------------
// Version range helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the version range contains any upper bound constraint
/// (`Lt`, `Lte`, or `MajorBound`).
fn has_upper_bound(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => false,
        VersionRange::Eq(_) => true, // exact version is both upper and lower
        VersionRange::Gt(_) | VersionRange::Gte(_) => false,
        VersionRange::Lt(_) | VersionRange::Lte(_) => true,
        VersionRange::MajorBound(_) => true,
        VersionRange::And(a, b) => has_upper_bound(a) || has_upper_bound(b),
        VersionRange::Or(a, b) => has_upper_bound(a) && has_upper_bound(b),
        VersionRange::NoVersion => true, // no versions match, vacuously bounded
    }
}

/// Returns `true` if the version range contains any lower bound constraint
/// (`Gt`, `Gte`, `Eq`, or `MajorBound`).
fn has_lower_bound(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => false,
        VersionRange::Eq(_) => true,
        VersionRange::Gt(_) | VersionRange::Gte(_) => true,
        VersionRange::Lt(_) | VersionRange::Lte(_) => false,
        VersionRange::MajorBound(_) => true,
        VersionRange::And(a, b) => has_lower_bound(a) || has_lower_bound(b),
        VersionRange::Or(a, b) => has_lower_bound(a) && has_lower_bound(b),
        VersionRange::NoVersion => true, // vacuously bounded
    }
}

/// Returns `true` if the version range is effectively "any version" — i.e.
/// `>=0` or completely unconstrained.
fn is_wide_any(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => true,
        VersionRange::Gte(v) => v.components.iter().all(|&c| c == 0),
        VersionRange::Gt(v) => v.components.iter().all(|&c| c == 0),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Condition helpers
// ---------------------------------------------------------------------------

/// Collect all flag names referenced in a condition tree.
fn collect_flag_refs<'a>(condition: &Condition<'a>, flags: &mut Vec<&'a str>) {
    match condition {
        Condition::Flag(name) => flags.push(name),
        Condition::Not(inner) => collect_flag_refs(inner, flags),
        Condition::And(a, b) | Condition::Or(a, b) => {
            collect_flag_refs(a, flags);
            collect_flag_refs(b, flags);
        }
        Condition::OS(_)
        | Condition::Arch(_)
        | Condition::Impl(_, _)
        | Condition::Raw(_)
        | Condition::Lit(_) => {}
    }
}

/// Collect all flag references from conditionals recursively.
fn collect_flags_from_conditionals<'a>(conditionals: &[Conditional<'a>], flags: &mut Vec<&'a str>) {
    for cond in conditionals {
        collect_flag_refs(&cond.condition, flags);
        collect_flags_from_conditionals(&cond.then_conditionals, flags);
        collect_flags_from_conditionals(&cond.else_conditionals, flags);
    }
}

/// Span helper: get a span from a CST node id, falling back to empty span.
///
/// We don't have direct access to the CST here, so we use an empty span as a
/// fallback. In a full integration the caller would resolve `NodeId` to `Span`
/// via the CST.
fn span_for_node(_file: &CabalFile<'_>, _node: cabalist_parser::span::NodeId) -> Span {
    Span::empty(0)
}

// ---------------------------------------------------------------------------
// Individual lints
// ---------------------------------------------------------------------------

/// `missing-upper-bound`: Dependency has no upper version bound.
fn lint_missing_upper_bound(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-upper-bound";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    for dep in file.all_dependencies() {
        match &dep.version_range {
            None => {
                // No version range at all — that's covered by wide-any-version
            }
            Some(vr) => {
                if !has_upper_bound(vr) {
                    lints.push(Lint {
                        id: ID,
                        severity,
                        message: format!(
                            "Dependency '{}' has no upper version bound ({}). \
                             This violates the PVP and may break on future releases.",
                            dep.package, vr
                        ),
                        span: span_for_node(file, dep.cst_node),
                        suggestion: Some(
                            "Consider using '^>=' for PVP-compliant major bounds".to_string(),
                        ),
                    });
                }
            }
        }
    }
}

/// `missing-lower-bound`: Dependency has no lower version bound.
fn lint_missing_lower_bound(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-lower-bound";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    for dep in file.all_dependencies() {
        match &dep.version_range {
            None => {
                // No version range at all — covered by wide-any-version
            }
            Some(vr) => {
                if !has_lower_bound(vr) {
                    lints.push(Lint {
                        id: ID,
                        severity,
                        message: format!(
                            "Dependency '{}' has no lower version bound ({}).",
                            dep.package, vr
                        ),
                        span: span_for_node(file, dep.cst_node),
                        suggestion: Some(
                            "Add a lower bound to ensure a minimum compatible version".to_string(),
                        ),
                    });
                }
            }
        }
    }
}

/// `wide-any-version`: Dependency uses `>=0` or no constraint at all.
fn lint_wide_any_version(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "wide-any-version";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    for dep in file.all_dependencies() {
        let fires = match &dep.version_range {
            None => true,
            Some(vr) => is_wide_any(vr),
        };
        if fires {
            lints.push(Lint {
                id: ID,
                severity,
                message: format!(
                    "Dependency '{}' has no meaningful version constraint. \
                     Any version will be accepted, which is fragile.",
                    dep.package
                ),
                span: span_for_node(file, dep.cst_node),
                suggestion: Some("Add version bounds, e.g. '^>=X.Y'".to_string()),
            });
        }
    }
}

/// `missing-synopsis`: No `synopsis` field.
fn lint_missing_synopsis(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-synopsis";
    if !config.is_enabled(ID) {
        return;
    }
    if file.synopsis.is_none() {
        lints.push(Lint {
            id: ID,
            severity: config.effective_severity(ID, Severity::Info),
            message: "Package is missing a 'synopsis' field.".to_string(),
            span: Span::empty(0),
            suggestion: Some("Add a one-line synopsis describing your package".to_string()),
        });
    }
}

/// `missing-description`: No `description` field.
fn lint_missing_description(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-description";
    if !config.is_enabled(ID) {
        return;
    }
    if file.description.is_none() {
        lints.push(Lint {
            id: ID,
            severity: config.effective_severity(ID, Severity::Info),
            message: "Package is missing a 'description' field.".to_string(),
            span: Span::empty(0),
            suggestion: Some("Add a description for your package's Hackage page".to_string()),
        });
    }
}

/// `missing-source-repo`: No `source-repository` section.
fn lint_missing_source_repo(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-source-repo";
    if !config.is_enabled(ID) {
        return;
    }
    if file.source_repositories.is_empty() {
        lints.push(Lint {
            id: ID,
            severity: config.effective_severity(ID, Severity::Info),
            message: "Package has no 'source-repository' section.".to_string(),
            span: Span::empty(0),
            suggestion: Some(
                "Add a 'source-repository head' section with your VCS URL".to_string(),
            ),
        });
    }
}

/// `missing-bug-reports`: No `bug-reports` field.
fn lint_missing_bug_reports(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-bug-reports";
    if !config.is_enabled(ID) {
        return;
    }
    if file.bug_reports.is_none() {
        lints.push(Lint {
            id: ID,
            severity: config.effective_severity(ID, Severity::Info),
            message: "Package is missing a 'bug-reports' field.".to_string(),
            span: Span::empty(0),
            suggestion: Some(
                "Add a 'bug-reports' URL so users know where to report issues".to_string(),
            ),
        });
    }
}

/// `ghc-options-werror`: `-Werror` in a non-conditional block.
///
/// `-Werror` is fine in a conditional block (like `if flag(ci)`) but should
/// not appear as a top-level option because it breaks downstream builds when
/// new GHC warnings are added.
fn lint_ghc_options_werror(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "ghc-options-werror";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    let check_fields = |fields: &ComponentFields<'_>, component_desc: &str| -> Vec<Lint> {
        let mut result = Vec::new();
        for opt in &fields.ghc_options {
            if *opt == "-Werror" {
                result.push(Lint {
                    id: ID,
                    severity,
                    message: format!(
                        "'-Werror' found in {component_desc}'s ghc-options. \
                         This can break downstream builds when GHC adds new warnings."
                    ),
                    span: span_for_node(file, fields.cst_node),
                    suggestion: Some(
                        "Move '-Werror' into a conditional, e.g. 'if flag(ci)'".to_string(),
                    ),
                });
            }
        }
        result
    };

    // Check all components' top-level ghc-options (not inside conditionals).
    if let Some(ref lib) = file.library {
        lints.extend(check_fields(&lib.fields, "library"));
    }
    for lib in &file.named_libraries {
        let desc = format!("library '{}'", lib.fields.name.unwrap_or("unnamed"));
        lints.extend(check_fields(&lib.fields, &desc));
    }
    for exe in &file.executables {
        let desc = format!("executable '{}'", exe.fields.name.unwrap_or("unnamed"));
        lints.extend(check_fields(&exe.fields, &desc));
    }
    for ts in &file.test_suites {
        let desc = format!("test-suite '{}'", ts.fields.name.unwrap_or("unnamed"));
        lints.extend(check_fields(&ts.fields, &desc));
    }
    for bm in &file.benchmarks {
        let desc = format!("benchmark '{}'", bm.fields.name.unwrap_or("unnamed"));
        lints.extend(check_fields(&bm.fields, &desc));
    }
    // Also check common stanzas.
    for cs in &file.common_stanzas {
        let desc = format!("common stanza '{}'", cs.name);
        lints.extend(check_fields(&cs.fields, &desc));
    }
}

/// `missing-default-language`: Component has no `default-language`.
fn lint_missing_default_language(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "missing-default-language";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    let check = |fields: &ComponentFields<'_>, desc: &str, lints: &mut Vec<Lint>| {
        if fields.default_language.is_none() {
            lints.push(Lint {
                id: ID,
                severity,
                message: format!(
                    "{desc} has no 'default-language' field. \
                     Cabal will pick one, but it should be explicit."
                ),
                span: span_for_node(file, fields.cst_node),
                suggestion: Some("Add 'default-language: GHC2021' or 'Haskell2010'".to_string()),
            });
        }
    };

    if let Some(ref lib) = file.library {
        check(&lib.fields, "Library", lints);
    }
    for lib in &file.named_libraries {
        let desc = format!("Library '{}'", lib.fields.name.unwrap_or("unnamed"));
        check(&lib.fields, &desc, lints);
    }
    for exe in &file.executables {
        let desc = format!("Executable '{}'", exe.fields.name.unwrap_or("unnamed"));
        check(&exe.fields, &desc, lints);
    }
    for ts in &file.test_suites {
        let desc = format!("Test-suite '{}'", ts.fields.name.unwrap_or("unnamed"));
        check(&ts.fields, &desc, lints);
    }
    for bm in &file.benchmarks {
        let desc = format!("Benchmark '{}'", bm.fields.name.unwrap_or("unnamed"));
        check(&bm.fields, &desc, lints);
    }
}

/// `cabal-version-low`: `cabal-version < 3.0` — suggest upgrading.
fn lint_cabal_version_low(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "cabal-version-low";
    if !config.is_enabled(ID) {
        return;
    }

    if let Some(ref cv) = file.cabal_version {
        if let Some(ref v) = cv.version {
            let is_low = matches!(
                v.components.first(),
                Some(&major) if major < 3
            );
            if is_low {
                lints.push(Lint {
                    id: ID,
                    severity: config.effective_severity(ID, Severity::Info),
                    message: format!(
                        "cabal-version is {} — consider upgrading to 3.0 or later \
                         to unlock common stanzas and imports.",
                        v
                    ),
                    span: span_for_node(file, cv.cst_node),
                    suggestion: Some("Set 'cabal-version: 3.0'".to_string()),
                });
            }
        }
    }
}

/// `duplicate-dep`: Same package appears in `build-depends` more than once.
fn lint_duplicate_dep(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "duplicate-dep";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    let check = |fields: &ComponentFields<'_>, desc: &str, lints: &mut Vec<Lint>| {
        let mut seen = std::collections::HashSet::new();
        for dep in &fields.build_depends {
            let name_lower = dep.package.to_ascii_lowercase();
            if !seen.insert(name_lower.clone()) {
                lints.push(Lint {
                    id: ID,
                    severity,
                    message: format!(
                        "Duplicate dependency '{}' in {desc}'s build-depends.",
                        dep.package
                    ),
                    span: span_for_node(file, dep.cst_node),
                    suggestion: Some("Remove the duplicate entry".to_string()),
                });
            }
        }
    };

    if let Some(ref lib) = file.library {
        check(&lib.fields, "library", lints);
    }
    for lib in &file.named_libraries {
        let desc = format!("library '{}'", lib.fields.name.unwrap_or("unnamed"));
        check(&lib.fields, &desc, lints);
    }
    for exe in &file.executables {
        let desc = format!("executable '{}'", exe.fields.name.unwrap_or("unnamed"));
        check(&exe.fields, &desc, lints);
    }
    for ts in &file.test_suites {
        let desc = format!("test-suite '{}'", ts.fields.name.unwrap_or("unnamed"));
        check(&ts.fields, &desc, lints);
    }
    for bm in &file.benchmarks {
        let desc = format!("benchmark '{}'", bm.fields.name.unwrap_or("unnamed"));
        check(&bm.fields, &desc, lints);
    }
    for cs in &file.common_stanzas {
        let desc = format!("common stanza '{}'", cs.name);
        check(&cs.fields, &desc, lints);
    }
}

/// `unused-flag`: A `flag` section exists but is never referenced in conditions.
fn lint_unused_flag(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "unused-flag";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Warning);

    if file.flags.is_empty() {
        return;
    }

    // Collect all flag references from all conditionals in all components.
    let mut referenced_flags: Vec<String> = Vec::new();

    fn collect_flag_strings(fields: &ComponentFields<'_>, out: &mut Vec<String>) {
        let mut refs = Vec::new();
        collect_flags_from_conditionals(&fields.conditionals, &mut refs);
        out.extend(refs.iter().map(|s| s.to_ascii_lowercase()));
    }

    if let Some(ref lib) = file.library {
        collect_flag_strings(&lib.fields, &mut referenced_flags);
    }
    for lib in &file.named_libraries {
        collect_flag_strings(&lib.fields, &mut referenced_flags);
    }
    for exe in &file.executables {
        collect_flag_strings(&exe.fields, &mut referenced_flags);
    }
    for ts in &file.test_suites {
        collect_flag_strings(&ts.fields, &mut referenced_flags);
    }
    for bm in &file.benchmarks {
        collect_flag_strings(&bm.fields, &mut referenced_flags);
    }
    for cs in &file.common_stanzas {
        collect_flag_strings(&cs.fields, &mut referenced_flags);
    }

    // Deduplicate.
    let referenced_lower: std::collections::HashSet<String> =
        referenced_flags.into_iter().collect();

    for flag in &file.flags {
        if !referenced_lower.contains(&flag.name.to_ascii_lowercase()) {
            lints.push(Lint {
                id: ID,
                severity,
                message: format!(
                    "Flag '{}' is defined but never referenced in any conditional.",
                    flag.name
                ),
                span: span_for_node(file, flag.cst_node),
                suggestion: Some(
                    "Remove the unused flag or add a conditional that uses it".to_string(),
                ),
            });
        }
    }
}

/// `no-common-stanza`: Multiple sections share ≥5 identical field names,
/// suggesting the common parts should be extracted into a `common` stanza.
fn lint_no_common_stanza(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "no-common-stanza";
    if !config.is_enabled(ID) {
        return;
    }
    // Skip if common stanzas already exist — the user knows about them.
    if !file.common_stanzas.is_empty() {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Info);

    // Collect the set of field names present in each component.
    let mut component_field_sets: Vec<(&str, std::collections::HashSet<String>)> = Vec::new();

    let field_names_of = |fields: &ComponentFields<'_>| -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        if fields.default_language.is_some() {
            names.insert("default-language".to_string());
        }
        if !fields.ghc_options.is_empty() {
            names.insert("ghc-options".to_string());
        }
        if !fields.default_extensions.is_empty() {
            names.insert("default-extensions".to_string());
        }
        if !fields.build_depends.is_empty() {
            names.insert("build-depends".to_string());
        }
        if !fields.hs_source_dirs.is_empty() {
            names.insert("hs-source-dirs".to_string());
        }
        for f in &fields.other_fields {
            names.insert(f.name.to_ascii_lowercase().replace('_', "-"));
        }
        names
    };

    if let Some(ref lib) = file.library {
        component_field_sets.push(("library", field_names_of(&lib.fields)));
    }
    for exe in &file.executables {
        let desc = exe.fields.name.unwrap_or("unnamed");
        component_field_sets.push((desc, field_names_of(&exe.fields)));
    }
    for ts in &file.test_suites {
        let desc = ts.fields.name.unwrap_or("unnamed");
        component_field_sets.push((desc, field_names_of(&ts.fields)));
    }
    for bm in &file.benchmarks {
        let desc = bm.fields.name.unwrap_or("unnamed");
        component_field_sets.push((desc, field_names_of(&bm.fields)));
    }

    // Need at least 2 components to compare.
    if component_field_sets.len() < 2 {
        return;
    }

    // Find the intersection of all component field sets.
    let mut common: std::collections::HashSet<String> = component_field_sets[0].1.clone();
    for (_, fields) in &component_field_sets[1..] {
        common = common.intersection(fields).cloned().collect();
    }

    if common.len() >= 5 {
        let mut shared: Vec<&str> = common.iter().map(|s| s.as_str()).collect();
        shared.sort();
        lints.push(Lint {
            id: ID,
            severity,
            message: format!(
                "{} components share {} common fields ({}). \
                 Consider extracting a 'common' stanza to reduce duplication.",
                component_field_sets.len(),
                common.len(),
                shared.join(", "),
            ),
            span: Span::empty(0),
            suggestion: Some(
                "Create a 'common warnings' stanza and use 'import: warnings' in each component"
                    .to_string(),
            ),
        });
    }
}

/// `exposed-no-modules`: Library with empty or missing `exposed-modules`.
fn lint_exposed_no_modules(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "exposed-no-modules";
    if !config.is_enabled(ID) {
        return;
    }
    let severity = config.effective_severity(ID, Severity::Error);

    if let Some(ref lib) = file.library {
        if lib.exposed_modules.is_empty() {
            lints.push(Lint {
                id: ID,
                severity,
                message: "Library has no 'exposed-modules'. \
                          A library must expose at least one module."
                    .to_string(),
                span: span_for_node(file, lib.fields.cst_node),
                suggestion: Some(
                    "Add 'exposed-modules: MyModule' to the library section".to_string(),
                ),
            });
        }
    }
    for lib in &file.named_libraries {
        if lib.exposed_modules.is_empty() {
            let name = lib.fields.name.unwrap_or("unnamed");
            lints.push(Lint {
                id: ID,
                severity,
                message: format!(
                    "Library '{name}' has no 'exposed-modules'. \
                     A library must expose at least one module."
                ),
                span: span_for_node(file, lib.fields.cst_node),
                suggestion: Some("Add 'exposed-modules' to this library section".to_string()),
            });
        }
    }
}

/// `stale-tested-with`: `tested-with` lists a GHC version more than 2 major
/// releases old.
fn lint_stale_tested_with(file: &CabalFile<'_>, config: &LintConfig, lints: &mut Vec<Lint>) {
    const ID: &str = "stale-tested-with";
    if !config.is_enabled(ID) {
        return;
    }

    // Current GHC major version baseline (9.12 as of early 2026).
    // "More than 2 major releases old" means < 9.8.
    const CURRENT_GHC_MAJOR: (u64, u64) = (9, 12);
    const STALE_THRESHOLD: u64 = 2; // 2 minor releases behind the major series

    let Some(ref tested_with) = file.tested_with else {
        return;
    };

    let severity = config.effective_severity(ID, Severity::Info);

    // Parse out GHC version references from the tested-with value.
    // Format is like: "GHC ==9.8.2, GHC ==9.6.4, GHC ==8.10.7"
    for segment in tested_with.split(',') {
        let segment = segment.trim();
        // Strip the "GHC" prefix and any comparison operators.
        let version_part = segment
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .trim()
            .trim_start_matches(|c: char| c == '=' || c == '>' || c == '<' || c == '^')
            .trim();

        if version_part.is_empty() {
            continue;
        }

        // Parse major.minor from the version string.
        let parts: Vec<&str> = version_part.split('.').collect();
        let major: Option<u64> = parts.first().and_then(|s| s.parse().ok());
        let minor: Option<u64> = parts.get(1).and_then(|s| s.parse().ok());

        if let (Some(major), Some(minor)) = (major, minor) {
            // Compute how many minor releases behind this version is.
            // GHC versioning: 9.6, 9.8, 9.10, 9.12 — minor bumps by 2.
            let is_stale = if major < CURRENT_GHC_MAJOR.0 {
                true
            } else if major == CURRENT_GHC_MAJOR.0 {
                // Each GHC release bumps minor by 2, so "2 releases back"
                // means minor <= current_minor - 4.
                CURRENT_GHC_MAJOR.1.saturating_sub(minor) > STALE_THRESHOLD * 2
            } else {
                false
            };

            if is_stale {
                lints.push(Lint {
                    id: ID,
                    severity,
                    message: format!(
                        "'tested-with' lists GHC {major}.{minor}, which is more than \
                         {STALE_THRESHOLD} major releases behind the current series \
                         ({}.{}).",
                        CURRENT_GHC_MAJOR.0, CURRENT_GHC_MAJOR.1,
                    ),
                    span: Span::empty(0),
                    suggestion: Some(
                        "Consider updating 'tested-with' to reflect currently supported GHC versions"
                            .to_string(),
                    ),
                });
            }
        }
    }
}

/// List of all lint IDs recognized by this module.
pub const ALL_LINT_IDS: &[&str] = &[
    "missing-upper-bound",
    "missing-lower-bound",
    "wide-any-version",
    "missing-synopsis",
    "missing-description",
    "missing-source-repo",
    "missing-bug-reports",
    "no-common-stanza",
    "ghc-options-werror",
    "missing-default-language",
    "exposed-no-modules",
    "cabal-version-low",
    "duplicate-dep",
    "unused-flag",
    "stale-tested-with",
];

#[cfg(test)]
mod tests {
    use super::*;
    use cabalist_parser::ast::Version;
    use cabalist_parser::{ast::derive_ast, parse};

    fn parse_and_lint(source: &str) -> Vec<Lint> {
        let result = parse(source);
        let ast = derive_ast(&result.cst);
        run_lints(&ast, &LintConfig::default())
    }

    fn parse_and_lint_with_config(source: &str, config: &LintConfig) -> Vec<Lint> {
        let result = parse(source);
        let ast = derive_ast(&result.cst);
        run_lints(&ast, config)
    }

    fn lint_ids(lints: &[Lint]) -> Vec<&str> {
        lints.iter().map(|l| l.id).collect()
    }

    #[test]
    fn missing_synopsis_fires() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-synopsis"));
    }

    #[test]
    fn missing_synopsis_does_not_fire_when_present() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\nsynopsis: A package\n";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"missing-synopsis"));
    }

    #[test]
    fn missing_description_fires() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-description"));
    }

    #[test]
    fn missing_source_repo_fires() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-source-repo"));
    }

    #[test]
    fn missing_bug_reports_fires() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-bug-reports"));
    }

    #[test]
    fn cabal_version_low_fires() {
        let source = "cabal-version: 2.4\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"cabal-version-low"));
    }

    #[test]
    fn cabal_version_low_does_not_fire_for_3_0() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"cabal-version-low"));
    }

    #[test]
    fn missing_default_language_fires() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  exposed-modules: Foo
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-default-language"));
    }

    #[test]
    fn missing_default_language_does_not_fire_when_present() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  exposed-modules: Foo
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"missing-default-language"));
    }

    #[test]
    fn missing_upper_bound_fires() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base >=4.14
  exposed-modules: Foo
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"missing-upper-bound"));
    }

    #[test]
    fn missing_upper_bound_does_not_fire_for_major_bound() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  exposed-modules: Foo
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"missing-upper-bound"));
    }

    #[test]
    fn wide_any_version_fires_for_no_constraint() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base
  exposed-modules: Foo
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"wide-any-version"));
    }

    #[test]
    fn ghc_options_werror_fires() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  exposed-modules: Foo
  default-language: GHC2021
  ghc-options: -Wall -Werror
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"ghc-options-werror"));
    }

    #[test]
    fn lint_disabled_via_config() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let config = LintConfig {
            disabled: vec!["missing-synopsis".to_string()],
            ..Default::default()
        };
        let lints = parse_and_lint_with_config(source, &config);
        assert!(!lint_ids(&lints).contains(&"missing-synopsis"));
    }

    #[test]
    fn lint_promoted_to_error() {
        let source = "cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n";
        let config = LintConfig {
            errors: vec!["missing-synopsis".to_string()],
            ..Default::default()
        };
        let lints = parse_and_lint_with_config(source, &config);
        let synopsis_lint = lints.iter().find(|l| l.id == "missing-synopsis");
        assert!(synopsis_lint.is_some());
        assert_eq!(synopsis_lint.unwrap().severity, Severity::Error);
    }

    #[test]
    fn version_range_helpers() {
        // ^>=1.0 has both bounds
        let vr = VersionRange::MajorBound(Version {
            components: vec![1, 0],
        });
        assert!(has_upper_bound(&vr));
        assert!(has_lower_bound(&vr));

        // >=1.0 has only lower bound
        let vr = VersionRange::Gte(Version {
            components: vec![1, 0],
        });
        assert!(!has_upper_bound(&vr));
        assert!(has_lower_bound(&vr));

        // <2.0 has only upper bound
        let vr = VersionRange::Lt(Version {
            components: vec![2, 0],
        });
        assert!(has_upper_bound(&vr));
        assert!(!has_lower_bound(&vr));

        // >=1.0 && <2.0 has both
        let vr = VersionRange::And(
            Box::new(VersionRange::Gte(Version {
                components: vec![1, 0],
            })),
            Box::new(VersionRange::Lt(Version {
                components: vec![2, 0],
            })),
        );
        assert!(has_upper_bound(&vr));
        assert!(has_lower_bound(&vr));

        // >=0 is wide
        let vr = VersionRange::Gte(Version {
            components: vec![0],
        });
        assert!(is_wide_any(&vr));

        // Any is wide
        assert!(is_wide_any(&VersionRange::Any));
    }

    #[test]
    fn all_lint_ids_list_is_complete() {
        assert_eq!(ALL_LINT_IDS.len(), 15);
    }

    #[test]
    fn exposed_no_modules_fires() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"exposed-no-modules"));
    }

    #[test]
    fn exposed_no_modules_does_not_fire_when_present() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  exposed-modules: Foo
  default-language: GHC2021
";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"exposed-no-modules"));
    }

    #[test]
    fn stale_tested_with_fires() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0
tested-with: GHC ==8.10.7
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"stale-tested-with"));
    }

    #[test]
    fn stale_tested_with_does_not_fire_for_recent() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0
tested-with: GHC ==9.10.1
";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"stale-tested-with"));
    }

    #[test]
    fn no_common_stanza_fires_when_sections_share_fields() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base ^>=4.17
  default-language: GHC2021
  ghc-options: -Wall
  default-extensions: OverloadedStrings
  exposed-modules: Foo
  hs-source-dirs: src

executable my-exe
  build-depends: base ^>=4.17
  default-language: GHC2021
  ghc-options: -Wall
  default-extensions: OverloadedStrings
  main-is: Main.hs
  hs-source-dirs: app
";
        let lints = parse_and_lint(source);
        assert!(lint_ids(&lints).contains(&"no-common-stanza"));
    }

    #[test]
    fn no_common_stanza_does_not_fire_when_common_stanza_exists() {
        let source = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

common warnings
  ghc-options: -Wall
  default-language: GHC2021

library
  import: warnings
  build-depends: base ^>=4.17
  exposed-modules: Foo

executable my-exe
  import: warnings
  build-depends: base ^>=4.17
  main-is: Main.hs
";
        let lints = parse_and_lint(source);
        assert!(!lint_ids(&lints).contains(&"no-common-stanza"));
    }
}
