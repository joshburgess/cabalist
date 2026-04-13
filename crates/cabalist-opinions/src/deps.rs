//! Recommended packages database.
//!
//! A curated map of "blessed" packages for common tasks, loaded from an
//! embedded TOML data file. The TUI search can surface these: when a user
//! searches for "json", we show `aeson` with a "Recommended" badge and a note
//! explaining why.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A recommended package category with its primary recommendation and
/// alternatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageCategory {
    /// Human-readable category name.
    pub name: String,
    /// The recommended package for this category.
    pub recommended: String,
    /// Companion packages that work well with the recommendation.
    #[serde(default)]
    pub companions: Vec<String>,
    /// Alternative packages the user might consider.
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Explanatory note about why this is recommended.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize)]
struct RecommendedDepsFile {
    category: BTreeMap<String, PackageCategory>,
}

const RECOMMENDED_DEPS_TOML: &str = include_str!("../data/recommended-deps.toml");

static RECOMMENDED: Lazy<BTreeMap<String, PackageCategory>> = Lazy::new(|| {
    let file: RecommendedDepsFile = toml::from_str(RECOMMENDED_DEPS_TOML)
        .expect("Failed to parse embedded recommended-deps.toml");
    file.category
});

/// Load all recommended package categories.
///
/// Returns a list of `(category_key, category)` pairs sorted by key.
pub fn load_recommended() -> Vec<(String, PackageCategory)> {
    RECOMMENDED
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Look up a package category by key (e.g., `"json"`, `"http-client"`).
pub fn category_info(key: &str) -> Option<&'static PackageCategory> {
    RECOMMENDED.get(key)
}

/// Check if a package is recommended, and if so, for what category.
///
/// Returns the category key (e.g., `"json"` for `aeson`).
pub fn is_recommended(package_name: &str) -> Option<&'static str> {
    for (key, cat) in RECOMMENDED.iter() {
        if cat.recommended == package_name {
            return Some(key.as_str());
        }
    }
    None
}

/// Check if a package is an alternative in some category.
///
/// Returns `(category_key, recommended_package)` if found.
pub fn is_alternative(package_name: &str) -> Option<(&'static str, &'static str)> {
    for (key, cat) in RECOMMENDED.iter() {
        if cat.alternatives.iter().any(|a| a == package_name) {
            return Some((key.as_str(), cat.recommended.as_str()));
        }
    }
    None
}

/// Check if a package is a companion to a recommended package.
///
/// Returns `(category_key, recommended_package)` if found.
pub fn is_companion(package_name: &str) -> Option<(&'static str, &'static str)> {
    for (key, cat) in RECOMMENDED.iter() {
        if cat.companions.iter().any(|c| c == package_name) {
            return Some((key.as_str(), cat.recommended.as_str()));
        }
    }
    None
}

/// Return all category keys.
pub fn all_category_keys() -> Vec<&'static str> {
    RECOMMENDED.keys().map(|k| k.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_recommended_has_enough_categories() {
        let cats = load_recommended();
        assert!(
            cats.len() >= 15,
            "Expected at least 15 categories, got {}",
            cats.len()
        );
    }

    #[test]
    fn category_info_json() {
        let cat = category_info("json").expect("json category should exist");
        assert_eq!(cat.recommended, "aeson");
        assert_eq!(cat.name, "JSON");
    }

    #[test]
    fn is_recommended_aeson() {
        assert_eq!(is_recommended("aeson"), Some("json"));
    }

    #[test]
    fn is_recommended_unknown() {
        assert_eq!(is_recommended("unknown-package-xyz"), None);
    }

    #[test]
    fn is_alternative_hspec() {
        let result = is_alternative("hspec");
        assert!(result.is_some());
        let (cat, rec) = result.unwrap();
        assert_eq!(cat, "testing");
        assert_eq!(rec, "tasty");
    }

    #[test]
    fn is_alternative_unknown() {
        assert_eq!(is_alternative("unknown-package-xyz"), None);
    }

    #[test]
    fn is_companion_stm() {
        let result = is_companion("stm");
        assert!(result.is_some());
        let (cat, rec) = result.unwrap();
        assert_eq!(cat, "concurrency");
        assert_eq!(rec, "async");
    }

    #[test]
    fn all_keys_non_empty() {
        let keys = all_category_keys();
        assert!(!keys.is_empty());
        for key in &keys {
            assert!(!key.is_empty());
        }
    }
}
