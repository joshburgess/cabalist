//! GHC language extension database.
//!
//! Provides a queryable database of GHC language extensions loaded from
//! an embedded TOML data file. Supports lookup by name (case-insensitive),
//! filtering by GHC version, and querying recommended defaults.

use crate::versions;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// A GHC language extension with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    /// The extension name as used in `{-# LANGUAGE ... #-}` or `.cabal` files.
    pub name: String,
    /// The GHC version that introduced this extension (e.g., "8.6.1").
    pub since: String,
    /// A brief human-readable description.
    pub description: String,
    /// Classification category for grouping/filtering.
    pub category: String,
    /// Whether the extension is generally safe to enable project-wide.
    pub safe: bool,
    /// Whether cabalist recommends this as a default extension.
    pub recommended: bool,
    /// Optional warning note for extensions with gotchas.
    #[serde(default)]
    pub warn: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtensionsFile {
    extension: Vec<Extension>,
}

const EXTENSIONS_TOML: &str = include_str!("../../../data/ghc-extensions.toml");

static EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
    let file: ExtensionsFile =
        toml::from_str(EXTENSIONS_TOML).expect("Failed to parse embedded ghc-extensions.toml");
    file.extension
});

/// Load all extensions from the embedded database.
///
/// This parses the TOML data on first call and caches the result.
pub fn load_extensions() -> &'static [Extension] {
    &EXTENSIONS
}

/// Filter extensions to those available for a given GHC version.
///
/// Returns extensions whose `since` version is `<=` the given GHC version.
pub fn extensions_for_ghc(ghc_version: &str) -> Vec<&'static Extension> {
    EXTENSIONS
        .iter()
        .filter(|ext| versions::version_gte(ghc_version, &ext.since))
        .collect()
}

/// Get the recommended default extensions set (names only).
pub fn default_extensions() -> Vec<&'static str> {
    EXTENSIONS
        .iter()
        .filter(|ext| ext.recommended)
        .map(|ext| ext.name.as_str())
        .collect()
}

/// Look up extension info by name (case-insensitive).
pub fn extension_info(name: &str) -> Option<&'static Extension> {
    EXTENSIONS
        .iter()
        .find(|ext| ext.name.eq_ignore_ascii_case(name))
}

/// Get all extension categories present in the database.
pub fn categories() -> Vec<&'static str> {
    let mut cats: Vec<&str> = EXTENSIONS.iter().map(|ext| ext.category.as_str()).collect();
    cats.sort_unstable();
    cats.dedup();
    cats
}

/// Get all extensions in a given category.
pub fn extensions_in_category(category: &str) -> Vec<&'static Extension> {
    EXTENSIONS
        .iter()
        .filter(|ext| ext.category.eq_ignore_ascii_case(category))
        .collect()
}

/// Get all extensions marked as safe to enable project-wide.
pub fn safe_extensions() -> Vec<&'static Extension> {
    EXTENSIONS.iter().filter(|ext| ext.safe).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_extensions_has_many_entries() {
        let exts = load_extensions();
        assert!(
            exts.len() >= 100,
            "Expected at least 100 extensions, got {}",
            exts.len()
        );
    }

    #[test]
    fn extension_info_by_name() {
        let ext = extension_info("OverloadedStrings").expect("OverloadedStrings should exist");
        assert_eq!(ext.name, "OverloadedStrings");
        assert_eq!(ext.since, "6.8.1");
        assert!(ext.safe);
        assert!(ext.recommended);
    }

    #[test]
    fn extension_info_case_insensitive() {
        assert!(extension_info("overloadedstrings").is_some());
        assert!(extension_info("OVERLOADEDSTRINGS").is_some());
        assert!(extension_info("derivingstrategies").is_some());
    }

    #[test]
    fn extension_info_nonexistent() {
        assert!(extension_info("NonExistentExtension123").is_none());
    }

    #[test]
    fn extensions_for_ghc_filters_correctly() {
        // DerivingVia was introduced in GHC 8.6.1
        let exts_old = extensions_for_ghc("8.4.4");
        assert!(
            !exts_old.iter().any(|e| e.name == "DerivingVia"),
            "DerivingVia should not be available before 8.6.1"
        );

        let exts_new = extensions_for_ghc("8.6.1");
        assert!(
            exts_new.iter().any(|e| e.name == "DerivingVia"),
            "DerivingVia should be available from 8.6.1"
        );
    }

    #[test]
    fn extensions_for_ghc_overloaded_record_dot() {
        // OverloadedRecordDot since 9.2.1
        let exts_before = extensions_for_ghc("9.0.2");
        assert!(!exts_before.iter().any(|e| e.name == "OverloadedRecordDot"));

        let exts_after = extensions_for_ghc("9.2.1");
        assert!(exts_after.iter().any(|e| e.name == "OverloadedRecordDot"));
    }

    #[test]
    fn extensions_for_ghc_import_qualified_post() {
        // ImportQualifiedPost since 8.10.1
        let exts_before = extensions_for_ghc("8.8.4");
        assert!(!exts_before.iter().any(|e| e.name == "ImportQualifiedPost"));

        let exts_after = extensions_for_ghc("8.10.1");
        assert!(exts_after.iter().any(|e| e.name == "ImportQualifiedPost"));
    }

    #[test]
    fn default_extensions_are_reasonable() {
        let defaults = default_extensions();
        assert!(!defaults.is_empty());
        assert!(defaults.contains(&"OverloadedStrings"));
        assert!(defaults.contains(&"DerivingStrategies"));
        assert!(defaults.contains(&"DeriveGeneric"));
        assert!(defaults.contains(&"LambdaCase"));
        assert!(defaults.contains(&"ScopedTypeVariables"));
        // TemplateHaskell should NOT be a default
        assert!(!defaults.contains(&"TemplateHaskell"));
    }

    #[test]
    fn default_extensions_all_in_database() {
        let defaults = default_extensions();
        for name in &defaults {
            assert!(
                extension_info(name).is_some(),
                "Default extension {name} not found in database"
            );
        }
    }

    #[test]
    fn categories_not_empty() {
        let cats = categories();
        assert!(!cats.is_empty());
        assert!(cats.contains(&"types"));
        assert!(cats.contains(&"deriving"));
        assert!(cats.contains(&"syntax"));
    }

    #[test]
    fn extensions_in_category_types() {
        let type_exts = extensions_in_category("types");
        assert!(!type_exts.is_empty());
        assert!(type_exts.iter().any(|e| e.name == "DataKinds"));
        assert!(type_exts.iter().any(|e| e.name == "GADTs"));
    }

    #[test]
    fn template_haskell_has_warning() {
        let ext = extension_info("TemplateHaskell").unwrap();
        assert!(!ext.safe);
        assert!(!ext.recommended);
        assert!(ext.warn.is_some());
    }

    #[test]
    fn standalone_kind_signatures_since() {
        let ext = extension_info("StandaloneKindSignatures").unwrap();
        assert_eq!(ext.since, "8.10.1");
    }

    #[test]
    fn block_arguments_since() {
        let ext = extension_info("BlockArguments").unwrap();
        assert_eq!(ext.since, "8.6.1");
    }

    #[test]
    fn numeric_underscores_since() {
        let ext = extension_info("NumericUnderscores").unwrap();
        assert_eq!(ext.since, "8.6.1");
    }

    #[test]
    fn quantified_constraints_since() {
        let ext = extension_info("QuantifiedConstraints").unwrap();
        assert_eq!(ext.since, "8.6.1");
    }

    #[test]
    fn linear_types_since() {
        let ext = extension_info("LinearTypes").unwrap();
        assert_eq!(ext.since, "9.0.1");
    }

    #[test]
    fn safe_extensions_subset() {
        let safe = safe_extensions();
        // All safe extensions should have safe == true
        for ext in &safe {
            assert!(ext.safe, "{} should be safe", ext.name);
        }
        // Unsafe ones should not be in the list
        assert!(!safe.iter().any(|e| e.name == "UndecidableInstances"));
        assert!(!safe.iter().any(|e| e.name == "TemplateHaskell"));
    }
}
