//! GHC version detection and GHC-to-base library version mapping.

use serde::{Deserialize, Serialize};
use std::process::Command;

/// A mapping from a GHC version to the `base` library version it ships with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhcBaseMapping {
    pub ghc: &'static str,
    pub base: &'static str,
}

/// Static table of GHC version to `base` library version mappings.
///
/// Sorted from newest to oldest.
pub const GHC_BASE_MAP: &[GhcBaseMapping] = &[
    GhcBaseMapping {
        ghc: "9.12.1",
        base: "4.21.0.0",
    },
    GhcBaseMapping {
        ghc: "9.10.1",
        base: "4.20.0.0",
    },
    GhcBaseMapping {
        ghc: "9.8.4",
        base: "4.19.2.0",
    },
    GhcBaseMapping {
        ghc: "9.8.2",
        base: "4.19.1.0",
    },
    GhcBaseMapping {
        ghc: "9.8.1",
        base: "4.19.0.0",
    },
    GhcBaseMapping {
        ghc: "9.6.6",
        base: "4.18.2.1",
    },
    GhcBaseMapping {
        ghc: "9.6.5",
        base: "4.18.2.0",
    },
    GhcBaseMapping {
        ghc: "9.6.4",
        base: "4.18.1.0",
    },
    GhcBaseMapping {
        ghc: "9.6.3",
        base: "4.18.1.0",
    },
    GhcBaseMapping {
        ghc: "9.6.2",
        base: "4.18.0.0",
    },
    GhcBaseMapping {
        ghc: "9.6.1",
        base: "4.18.0.0",
    },
    GhcBaseMapping {
        ghc: "9.4.8",
        base: "4.17.2.1",
    },
    GhcBaseMapping {
        ghc: "9.4.7",
        base: "4.17.2.0",
    },
    GhcBaseMapping {
        ghc: "9.4.4",
        base: "4.17.0.0",
    },
    GhcBaseMapping {
        ghc: "9.2.8",
        base: "4.16.4.0",
    },
    GhcBaseMapping {
        ghc: "9.2.5",
        base: "4.16.4.0",
    },
    GhcBaseMapping {
        ghc: "9.0.2",
        base: "4.15.1.0",
    },
    GhcBaseMapping {
        ghc: "8.10.7",
        base: "4.14.3.0",
    },
    GhcBaseMapping {
        ghc: "8.8.4",
        base: "4.13.0.0",
    },
    GhcBaseMapping {
        ghc: "8.6.5",
        base: "4.12.0.0",
    },
    GhcBaseMapping {
        ghc: "8.4.4",
        base: "4.11.1.0",
    },
    GhcBaseMapping {
        ghc: "8.2.2",
        base: "4.10.1.0",
    },
    GhcBaseMapping {
        ghc: "8.0.2",
        base: "4.9.1.0",
    },
];

/// Return the static GHC-to-base version mapping table.
pub fn ghc_base_map() -> &'static [GhcBaseMapping] {
    GHC_BASE_MAP
}

/// Look up the `base` library version for a specific GHC version.
///
/// Returns an exact match, or `None` if the GHC version is not in the table.
pub fn base_version_for_ghc(ghc_version: &str) -> Option<&'static str> {
    GHC_BASE_MAP
        .iter()
        .find(|m| m.ghc == ghc_version)
        .map(|m| m.base)
}

/// Parse a version string like "9.8.2" into a vector of numeric components.
///
/// Non-numeric components cause parsing to stop at that point.
pub fn parse_version(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|s| s.parse::<u64>())
        .take_while(|r| r.is_ok())
        .map(|r| r.unwrap())
        .collect()
}

/// Compare two version strings, returning true if `v1 >= v2`.
pub fn version_gte(v1: &str, v2: &str) -> bool {
    let a = parse_version(v1);
    let b = parse_version(v2);
    a >= b
}

/// Compare two version strings, returning true if `v1 < v2`.
pub fn version_lt(v1: &str, v2: &str) -> bool {
    !version_gte(v1, v2)
}

/// Detect the installed GHC version by running `ghc --numeric-version`.
///
/// Returns `None` if `ghc` is not installed or the command fails.
pub fn detect_ghc_version() -> Option<String> {
    Command::new("ghc")
        .arg("--numeric-version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Detect the installed cabal-install version by running `cabal --numeric-version`.
///
/// Returns `None` if `cabal` is not installed or the command fails.
pub fn detect_cabal_version() -> Option<String> {
    Command::new("cabal")
        .arg("--numeric-version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Returns true if the given GHC version supports GHC2021 (>= 9.2).
pub fn supports_ghc2021(ghc_version: &str) -> bool {
    version_gte(ghc_version, "9.2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(parse_version("9.8.2"), vec![9, 8, 2]);
        assert_eq!(parse_version("8.10.7"), vec![8, 10, 7]);
        assert_eq!(parse_version("4.19.1.0"), vec![4, 19, 1, 0]);
    }

    #[test]
    fn parse_version_single() {
        assert_eq!(parse_version("9"), vec![9]);
    }

    #[test]
    fn parse_version_empty() {
        assert_eq!(parse_version(""), Vec::<u64>::new());
    }

    #[test]
    fn version_gte_basic() {
        assert!(version_gte("9.8.2", "9.8.1"));
        assert!(version_gte("9.8.2", "9.8.2"));
        assert!(!version_gte("9.8.1", "9.8.2"));
        assert!(version_gte("9.10.1", "9.8.2"));
        assert!(version_gte("10.0.1", "9.99.99"));
    }

    #[test]
    fn version_gte_different_lengths() {
        assert!(version_gte("9.2", "9.2"));
        assert!(version_gte("9.2.1", "9.2"));
        assert!(!version_gte("9.1", "9.2"));
    }

    #[test]
    fn base_version_lookup() {
        assert_eq!(base_version_for_ghc("9.8.2"), Some("4.19.1.0"));
        assert_eq!(base_version_for_ghc("9.10.1"), Some("4.20.0.0"));
        assert_eq!(base_version_for_ghc("8.10.7"), Some("4.14.3.0"));
        assert_eq!(base_version_for_ghc("8.6.5"), Some("4.12.0.0"));
        assert_eq!(base_version_for_ghc("99.99.99"), None);
    }

    #[test]
    fn ghc_base_map_not_empty() {
        assert!(!ghc_base_map().is_empty());
        assert!(ghc_base_map().len() >= 20);
    }

    #[test]
    fn ghc2021_support() {
        assert!(supports_ghc2021("9.2.1"));
        assert!(supports_ghc2021("9.8.2"));
        assert!(supports_ghc2021("9.2"));
        assert!(!supports_ghc2021("9.0.2"));
        assert!(!supports_ghc2021("8.10.7"));
    }
}
