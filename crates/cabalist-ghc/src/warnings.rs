//! GHC warning flags database.
//!
//! Provides a queryable database of GHC warning flags loaded from
//! an embedded TOML data file. Supports lookup by flag name, filtering
//! by group membership, and querying recommended warnings.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// A GHC warning flag with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Warning {
    /// The warning flag (e.g., "-Wfoo").
    pub flag: String,
    /// The GHC version that introduced this flag.
    pub since: String,
    /// A brief human-readable description.
    pub description: String,
    /// Which warning groups include this flag (e.g., `["-Wall"]`).
    #[serde(default)]
    pub group: Vec<String>,
    /// Whether cabalist recommends enabling this flag.
    #[serde(default)]
    pub recommended: bool,
    /// Optional list of contexts where this flag is recommended (e.g., `["ci"]`).
    #[serde(default)]
    pub recommended_for: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WarningsFile {
    warning: Vec<Warning>,
}

const WARNINGS_TOML: &str = include_str!("../data/ghc-warnings.toml");

static WARNINGS: Lazy<Vec<Warning>> = Lazy::new(|| {
    let file: WarningsFile =
        toml::from_str(WARNINGS_TOML).expect("Failed to parse embedded ghc-warnings.toml");
    file.warning
});

/// Load all warnings from the embedded database.
///
/// This parses the TOML data on first call and caches the result.
pub fn load_warnings() -> &'static [Warning] {
    &WARNINGS
}

/// Look up a warning by its flag name (e.g., "-Wall").
///
/// Comparison is exact (warning flags are case-sensitive).
pub fn warning_info(flag: &str) -> Option<&'static Warning> {
    WARNINGS.iter().find(|w| w.flag == flag)
}

/// Get all warnings that belong to a given group (e.g., "-Wall").
pub fn warnings_in_group(group: &str) -> Vec<&'static Warning> {
    WARNINGS
        .iter()
        .filter(|w| w.group.iter().any(|g| g == group))
        .collect()
}

/// Get all warnings marked as generally recommended.
pub fn recommended_warnings() -> Vec<&'static Warning> {
    WARNINGS.iter().filter(|w| w.recommended).collect()
}

/// Get all warnings recommended for a specific context (e.g., "ci").
pub fn warnings_recommended_for(context: &str) -> Vec<&'static Warning> {
    WARNINGS
        .iter()
        .filter(|w| w.recommended || w.recommended_for.iter().any(|c| c == context))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_warnings_has_many_entries() {
        let warns = load_warnings();
        assert!(
            warns.len() >= 30,
            "Expected at least 30 warnings, got {}",
            warns.len()
        );
    }

    #[test]
    fn warning_info_lookup() {
        let w = warning_info("-Wall").expect("-Wall should exist");
        assert_eq!(w.flag, "-Wall");
        assert!(w.recommended);
    }

    #[test]
    fn warning_info_nonexistent() {
        assert!(warning_info("-Wnonexistent-flag-xyz").is_none());
    }

    #[test]
    fn warnings_in_wall_group() {
        let wall = warnings_in_group("-Wall");
        assert!(!wall.is_empty(), "-Wall group should have members");

        let flags: Vec<&str> = wall.iter().map(|w| w.flag.as_str()).collect();
        assert!(flags.contains(&"-Wincomplete-patterns"));
        assert!(flags.contains(&"-Wunused-imports"));
        assert!(flags.contains(&"-Wunused-binds"));
        assert!(flags.contains(&"-Wmissing-signatures"));
        assert!(flags.contains(&"-Widentities"));
        assert!(flags.contains(&"-Wredundant-constraints"));

        // -Wall itself should NOT be listed as a member of -Wall
        assert!(!flags.contains(&"-Wall"));
    }

    #[test]
    fn warnings_in_wcompat_group() {
        let compat = warnings_in_group("-Wcompat");
        assert!(!compat.is_empty(), "-Wcompat group should have members");

        let flags: Vec<&str> = compat.iter().map(|w| w.flag.as_str()).collect();
        assert!(flags.contains(&"-Wstar-is-type"));
    }

    #[test]
    fn recommended_warnings_not_empty() {
        let rec = recommended_warnings();
        assert!(!rec.is_empty());

        let flags: Vec<&str> = rec.iter().map(|w| w.flag.as_str()).collect();
        assert!(flags.contains(&"-Wall"));
        assert!(flags.contains(&"-Wcompat"));
        assert!(flags.contains(&"-Wmissing-deriving-strategies"));
        assert!(flags.contains(&"-Wunused-packages"));
    }

    #[test]
    fn werror_recommended_for_ci() {
        let w = warning_info("-Werror").expect("-Werror should exist");
        assert!(
            !w.recommended,
            "-Werror should not be generally recommended"
        );
        assert!(
            w.recommended_for.contains(&"ci".to_string()),
            "-Werror should be recommended for CI"
        );
    }

    #[test]
    fn warnings_recommended_for_ci_includes_werror() {
        let ci_warns = warnings_recommended_for("ci");
        let flags: Vec<&str> = ci_warns.iter().map(|w| w.flag.as_str()).collect();
        assert!(
            flags.contains(&"-Werror"),
            "-Werror should be in CI recommendations"
        );
        // Also includes generally recommended ones
        assert!(flags.contains(&"-Wall"));
    }

    #[test]
    fn missing_deriving_strategies_since() {
        let w = warning_info("-Wmissing-deriving-strategies").unwrap();
        assert_eq!(w.since, "8.8.1");
    }

    #[test]
    fn unused_packages_since() {
        let w = warning_info("-Wunused-packages").unwrap();
        assert_eq!(w.since, "8.10.1");
    }

    #[test]
    fn partial_fields_recommended() {
        let w = warning_info("-Wpartial-fields").unwrap();
        assert!(w.recommended);
    }
}
