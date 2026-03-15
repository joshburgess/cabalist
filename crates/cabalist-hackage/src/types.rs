//! Core types for the Hackage interface.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A package version following PVP (Package Versioning Policy).
///
/// PVP versions have the form `A.B.C.D` where `A.B` is the major version,
/// `C` is the minor version, and `D` is the patch level. Not all components
/// are required — `1.0` is a valid version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub components: Vec<u64>,
}

impl Version {
    /// Create a new version from components.
    pub fn new(components: Vec<u64>) -> Self {
        Self { components }
    }

    /// Parse a version string like `"1.2.3.4"` into a `Version`.
    ///
    /// Returns `None` if the string is empty or contains non-numeric parts.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let components: Option<Vec<u64>> =
            s.split('.').map(|part| part.parse::<u64>().ok()).collect();
        let components = components?;
        if components.is_empty() {
            return None;
        }
        Some(Self { components })
    }

    /// Return the PVP major version `(A, B)`.
    ///
    /// If the version has only one component, the second is treated as `0`.
    pub fn major(&self) -> (u64, u64) {
        let a = self.components.first().copied().unwrap_or(0);
        let b = self.components.get(1).copied().unwrap_or(0);
        (a, b)
    }

    /// Return the number of components in this version.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Return true if this version has no components.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl std::hash::Hash for Version {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Normalize: skip trailing zeros so that [1,0] and [1,0,0] hash the same.
        let significant_len = self
            .components
            .iter()
            .rposition(|&c| c != 0)
            .map_or(0, |pos| pos + 1);
        self.components[..significant_len].hash(state);
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Version {}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.components.iter().map(|c| c.to_string()).collect();
        write!(f, "{}", parts.join("."))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let max_len = self.components.len().max(other.components.len());
        for i in 0..max_len {
            let a = self.components.get(i).copied().unwrap_or(0);
            let b = other.components.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        std::cmp::Ordering::Equal
    }
}

/// Information about a Hackage package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// The package name (e.g. `"aeson"`).
    pub name: String,
    /// A short description of the package.
    pub synopsis: String,
    /// All published versions, sorted ascending.
    pub versions: Vec<Version>,
    /// Whether this package is deprecated on Hackage.
    pub deprecated: bool,
}

impl PackageInfo {
    /// Return the latest (highest) version, if any.
    pub fn latest_version(&self) -> Option<&Version> {
        self.versions.iter().max()
    }
}

/// A version range in the Cabal/PVP sense.
///
/// Represents constraints on acceptable versions for a dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRange {
    /// Any version is acceptable.
    Any,
    /// Exact version match: `==A.B.C`.
    Eq(Version),
    /// Strictly greater than: `>A.B.C`.
    Gt(Version),
    /// Greater than or equal: `>=A.B.C`.
    Gte(Version),
    /// Strictly less than: `<A.B.C`.
    Lt(Version),
    /// Less than or equal: `<=A.B.C`.
    Lte(Version),
    /// PVP major bound: `^>=A.B` means `>=A.B && <A.(B+1)`.
    MajorBound(Version),
    /// Intersection of two ranges.
    And(Box<VersionRange>, Box<VersionRange>),
    /// Union of two ranges.
    Or(Box<VersionRange>, Box<VersionRange>),
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionRange::Any => write!(f, "-any"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        let v = Version::parse("1.2.3.4").unwrap();
        assert_eq!(v.components, vec![1, 2, 3, 4]);
    }

    #[test]
    fn parse_version_short() {
        let v = Version::parse("0.1").unwrap();
        assert_eq!(v.components, vec![0, 1]);
    }

    #[test]
    fn parse_version_single() {
        let v = Version::parse("5").unwrap();
        assert_eq!(v.components, vec![5]);
    }

    #[test]
    fn parse_version_whitespace() {
        let v = Version::parse("  1.2.3  ").unwrap();
        assert_eq!(v.components, vec![1, 2, 3]);
    }

    #[test]
    fn parse_version_empty() {
        assert!(Version::parse("").is_none());
        assert!(Version::parse("   ").is_none());
    }

    #[test]
    fn parse_version_invalid() {
        assert!(Version::parse("abc").is_none());
        assert!(Version::parse("1.2.abc").is_none());
        assert!(Version::parse("1..2").is_none());
    }

    #[test]
    fn version_display() {
        let v = Version::new(vec![2, 2, 1, 0]);
        assert_eq!(v.to_string(), "2.2.1.0");
    }

    #[test]
    fn version_display_short() {
        let v = Version::new(vec![1, 0]);
        assert_eq!(v.to_string(), "1.0");
    }

    #[test]
    fn version_major() {
        let v = Version::parse("4.14.3.0").unwrap();
        assert_eq!(v.major(), (4, 14));
    }

    #[test]
    fn version_major_short() {
        let v = Version::parse("5").unwrap();
        assert_eq!(v.major(), (5, 0));
    }

    #[test]
    fn version_ordering() {
        let v1 = Version::parse("2.2.0.0").unwrap();
        let v2 = Version::parse("2.2.1.0").unwrap();
        let v3 = Version::parse("2.3.0.0").unwrap();
        let v4 = Version::parse("2.2").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 == v4); // 2.2.0.0 == 2.2 (trailing zeros are implicit)
    }

    #[test]
    fn version_ordering_different_lengths() {
        let v1 = Version::parse("1.0").unwrap();
        let v2 = Version::parse("1.0.0.0").unwrap();
        assert_eq!(v1.cmp(&v2), std::cmp::Ordering::Equal);
    }

    #[test]
    fn package_info_latest_version() {
        let pkg = PackageInfo {
            name: "test".to_string(),
            synopsis: "a test package".to_string(),
            versions: vec![
                Version::parse("1.0").unwrap(),
                Version::parse("2.0").unwrap(),
                Version::parse("1.5").unwrap(),
            ],
            deprecated: false,
        };
        assert_eq!(pkg.latest_version(), Some(&Version::parse("2.0").unwrap()));
    }

    #[test]
    fn package_info_latest_version_empty() {
        let pkg = PackageInfo {
            name: "empty".to_string(),
            synopsis: "".to_string(),
            versions: vec![],
            deprecated: false,
        };
        assert_eq!(pkg.latest_version(), None);
    }

    #[test]
    fn version_range_display() {
        assert_eq!(VersionRange::Any.to_string(), "-any");
        assert_eq!(
            VersionRange::Gte(Version::parse("1.0").unwrap()).to_string(),
            ">=1.0"
        );
        assert_eq!(
            VersionRange::MajorBound(Version::parse("2.2").unwrap()).to_string(),
            "^>=2.2"
        );

        let range = VersionRange::And(
            Box::new(VersionRange::Gte(Version::parse("2.2").unwrap())),
            Box::new(VersionRange::Lt(Version::parse("2.3").unwrap())),
        );
        assert_eq!(range.to_string(), ">=2.2 && <2.3");
    }
}
