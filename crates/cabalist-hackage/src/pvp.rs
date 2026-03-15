//! PVP (Package Versioning Policy) version bound computation.
//!
//! This module contains pure logic for computing version bounds according
//! to the Haskell PVP. No network access is required.

use crate::types::{Version, VersionRange};

/// Compute PVP-compliant version bounds for a dependency.
///
/// Given a target version, returns `>= A.B.C.D && < A.(B+1)`.
/// This is the standard PVP upper bound: the major version `A.B` determines
/// the API contract, so we allow any version with the same `A.B` prefix.
///
/// # Examples
///
/// ```
/// use cabalist_hackage::pvp::compute_pvp_bounds;
/// use cabalist_hackage::types::Version;
///
/// let v = Version::parse("2.2.1.0").unwrap();
/// let range = compute_pvp_bounds(&v);
/// assert_eq!(range.to_string(), ">=2.2.1.0 && <2.3");
/// ```
pub fn compute_pvp_bounds(version: &Version) -> VersionRange {
    let upper = pvp_upper_bound(version);
    VersionRange::And(
        Box::new(VersionRange::Gte(version.clone())),
        Box::new(VersionRange::Lt(upper)),
    )
}

/// Compute the PVP upper bound version for a given version.
///
/// For version `A.B.C.D`, returns `A.(B+1)`.
fn pvp_upper_bound(version: &Version) -> Version {
    let (a, b) = version.major();
    Version::new(vec![a, b + 1])
}

/// Compute a major bound using the `^>=` operator.
///
/// `^>= A.B.C` means `>= A.B.C && < A.(B+1)` in PVP terms.
/// This returns the `MajorBound` variant which displays as `^>=A.B.C`.
///
/// # Examples
///
/// ```
/// use cabalist_hackage::pvp::compute_major_bound;
/// use cabalist_hackage::types::Version;
///
/// let v = Version::parse("2.2").unwrap();
/// let range = compute_major_bound(&v);
/// assert_eq!(range.to_string(), "^>=2.2");
/// ```
pub fn compute_major_bound(version: &Version) -> VersionRange {
    VersionRange::MajorBound(version.clone())
}

/// Check if a version satisfies a version range.
///
/// # Examples
///
/// ```
/// use cabalist_hackage::pvp::version_satisfies;
/// use cabalist_hackage::types::{Version, VersionRange};
///
/// let v = Version::parse("2.2.1.0").unwrap();
/// let range = VersionRange::Gte(Version::parse("2.2").unwrap());
/// assert!(version_satisfies(&v, &range));
/// ```
pub fn version_satisfies(version: &Version, range: &VersionRange) -> bool {
    match range {
        VersionRange::Any => true,
        VersionRange::Eq(v) => version == v,
        VersionRange::Gt(v) => version > v,
        VersionRange::Gte(v) => version >= v,
        VersionRange::Lt(v) => version < v,
        VersionRange::Lte(v) => version <= v,
        VersionRange::MajorBound(v) => {
            // ^>= A.B.C means >= A.B.C && < A.(B+1)
            let upper = pvp_upper_bound(v);
            version >= v && version < &upper
        }
        VersionRange::And(a, b) => version_satisfies(version, a) && version_satisfies(version, b),
        VersionRange::Or(a, b) => version_satisfies(version, a) || version_satisfies(version, b),
    }
}

/// Suggest PVP-compliant bounds given the version a user wants to use
/// and the list of all available versions.
///
/// Uses `^>=` (major bound) notation for the suggested version,
/// which is the most common and idiomatic form.
///
/// If there are no available versions, falls back to `compute_major_bound`.
pub fn suggest_bounds(current: &Version, available: &[Version]) -> VersionRange {
    if available.is_empty() {
        return compute_major_bound(current);
    }

    // Find the latest version in the same major series.
    let (a, b) = current.major();
    let same_major: Vec<&Version> = available
        .iter()
        .filter(|v| {
            let (va, vb) = v.major();
            va == a && vb == b
        })
        .collect();

    if same_major.is_empty() {
        // No other versions in the same major series — just use major bound.
        return compute_major_bound(current);
    }

    // Suggest ^>= with the current version — this is the most common
    // and PVP-idiomatic bound.
    compute_major_bound(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pvp_bounds_four_components() {
        let v = Version::parse("2.2.1.0").unwrap();
        let range = compute_pvp_bounds(&v);
        assert_eq!(range.to_string(), ">=2.2.1.0 && <2.3");
    }

    #[test]
    fn pvp_bounds_base_package() {
        let v = Version::parse("4.14.3.0").unwrap();
        let range = compute_pvp_bounds(&v);
        assert_eq!(range.to_string(), ">=4.14.3.0 && <4.15");
    }

    #[test]
    fn pvp_bounds_two_components() {
        let v = Version::parse("1.0").unwrap();
        let range = compute_pvp_bounds(&v);
        assert_eq!(range.to_string(), ">=1.0 && <1.1");
    }

    #[test]
    fn pvp_bounds_single_component() {
        let v = Version::parse("3").unwrap();
        let range = compute_pvp_bounds(&v);
        // major() returns (3, 0), so upper bound is 3.1
        assert_eq!(range.to_string(), ">=3 && <3.1");
    }

    #[test]
    fn pvp_bounds_zero_minor() {
        let v = Version::parse("1.0.0.0").unwrap();
        let range = compute_pvp_bounds(&v);
        assert_eq!(range.to_string(), ">=1.0.0.0 && <1.1");
    }

    #[test]
    fn major_bound_display() {
        let v = Version::parse("2.2").unwrap();
        let range = compute_major_bound(&v);
        assert_eq!(range.to_string(), "^>=2.2");
    }

    #[test]
    fn major_bound_with_patch() {
        let v = Version::parse("2.2.1.0").unwrap();
        let range = compute_major_bound(&v);
        assert_eq!(range.to_string(), "^>=2.2.1.0");
    }

    #[test]
    fn satisfies_gte() {
        let v = Version::parse("2.2.1.0").unwrap();
        let range = VersionRange::Gte(Version::parse("2.2").unwrap());
        assert!(version_satisfies(&v, &range));
    }

    #[test]
    fn satisfies_exact() {
        let v = Version::parse("2.2.1.0").unwrap();
        assert!(version_satisfies(
            &v,
            &VersionRange::Eq(Version::parse("2.2.1.0").unwrap())
        ));
        assert!(!version_satisfies(
            &v,
            &VersionRange::Eq(Version::parse("2.2.1.1").unwrap())
        ));
    }

    #[test]
    fn satisfies_pvp_range() {
        let range = compute_pvp_bounds(&Version::parse("2.2").unwrap());

        // In range
        assert!(version_satisfies(
            &Version::parse("2.2.0.0").unwrap(),
            &range
        ));
        assert!(version_satisfies(
            &Version::parse("2.2.1.0").unwrap(),
            &range
        ));
        assert!(version_satisfies(
            &Version::parse("2.2.99.0").unwrap(),
            &range
        ));

        // Out of range
        assert!(!version_satisfies(
            &Version::parse("2.3.0.0").unwrap(),
            &range
        ));
        assert!(!version_satisfies(
            &Version::parse("2.1.0.0").unwrap(),
            &range
        ));
        assert!(!version_satisfies(
            &Version::parse("3.0.0.0").unwrap(),
            &range
        ));
    }

    #[test]
    fn satisfies_major_bound() {
        let range = VersionRange::MajorBound(Version::parse("2.2.1.0").unwrap());

        assert!(version_satisfies(
            &Version::parse("2.2.1.0").unwrap(),
            &range
        ));
        assert!(version_satisfies(
            &Version::parse("2.2.3.0").unwrap(),
            &range
        ));
        assert!(!version_satisfies(
            &Version::parse("2.2.0.0").unwrap(),
            &range
        ));
        assert!(!version_satisfies(
            &Version::parse("2.3.0.0").unwrap(),
            &range
        ));
    }

    #[test]
    fn satisfies_or() {
        let range = VersionRange::Or(
            Box::new(VersionRange::Eq(Version::parse("1.0").unwrap())),
            Box::new(VersionRange::Eq(Version::parse("2.0").unwrap())),
        );
        assert!(version_satisfies(&Version::parse("1.0").unwrap(), &range));
        assert!(version_satisfies(&Version::parse("2.0").unwrap(), &range));
        assert!(!version_satisfies(&Version::parse("3.0").unwrap(), &range));
    }

    #[test]
    fn satisfies_any() {
        assert!(version_satisfies(
            &Version::parse("99.99.99").unwrap(),
            &VersionRange::Any
        ));
    }

    #[test]
    fn satisfies_gt_lt() {
        let v = Version::parse("2.0").unwrap();
        assert!(version_satisfies(
            &v,
            &VersionRange::Gt(Version::parse("1.0").unwrap())
        ));
        assert!(!version_satisfies(
            &v,
            &VersionRange::Gt(Version::parse("2.0").unwrap())
        ));
        assert!(version_satisfies(
            &v,
            &VersionRange::Lt(Version::parse("3.0").unwrap())
        ));
        assert!(!version_satisfies(
            &v,
            &VersionRange::Lt(Version::parse("2.0").unwrap())
        ));
        assert!(version_satisfies(
            &v,
            &VersionRange::Lte(Version::parse("2.0").unwrap())
        ));
    }

    #[test]
    fn suggest_bounds_empty_available() {
        let v = Version::parse("2.2.1.0").unwrap();
        let range = suggest_bounds(&v, &[]);
        assert_eq!(range, VersionRange::MajorBound(v));
    }

    #[test]
    fn suggest_bounds_with_available() {
        let v = Version::parse("2.2.1.0").unwrap();
        let available = vec![
            Version::parse("2.0.0.0").unwrap(),
            Version::parse("2.1.0.0").unwrap(),
            Version::parse("2.2.0.0").unwrap(),
            Version::parse("2.2.1.0").unwrap(),
            Version::parse("2.2.3.0").unwrap(),
            Version::parse("2.3.0.0").unwrap(),
        ];
        let range = suggest_bounds(&v, &available);
        assert_eq!(range, VersionRange::MajorBound(v));
    }
}
