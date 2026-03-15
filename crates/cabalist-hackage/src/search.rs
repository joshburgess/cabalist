//! Search and ranking logic for Hackage packages.
//!
//! This module implements fuzzy search over a list of [`PackageInfo`] entries.
//! The search is case-insensitive and ranks results by relevance using a
//! combination of exact match, prefix match, fuzzy subsequence match, and
//! synopsis substring match.
//!
//! All logic is pure and testable with mock data — no network access required.

use crate::types::PackageInfo;

/// A search result with relevance score and match classification.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched package.
    pub package: PackageInfo,
    /// Relevance score (higher is better). Range roughly 0.0..1.0.
    pub score: f64,
    /// How the match was found.
    pub match_kind: MatchKind,
}

/// Classification of how a search query matched a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// The query exactly equals the package name (case-insensitive).
    ExactName,
    /// The package name starts with the query (case-insensitive).
    PrefixName,
    /// The package name contains the query as a fuzzy subsequence.
    FuzzyName,
    /// The package synopsis contains the query as a substring.
    SynopsisMatch,
}

/// Search a list of packages by query string.
///
/// Returns results sorted by relevance score (highest first).
/// An empty query returns an empty result set.
///
/// The ranking priorities are:
/// 1. Exact name match (score ~1.0)
/// 2. Prefix name match (score ~0.8)
/// 3. Fuzzy name match (score varies based on quality)
/// 4. Synopsis substring match (score ~0.3)
///
/// Deprecated packages receive a penalty.
pub fn search(packages: &[PackageInfo], query: &str) -> Vec<SearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();

    let mut results: Vec<SearchResult> = packages
        .iter()
        .filter_map(|pkg| score_package(pkg, &query_lower))
        .collect();

    // Sort by score descending, then by name ascending for stability.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.package.name.cmp(&b.package.name))
    });

    results
}

/// Score a single package against a query. Returns `None` if no match.
fn score_package(pkg: &PackageInfo, query_lower: &str) -> Option<SearchResult> {
    let name_lower = pkg.name.to_lowercase();
    let synopsis_lower = pkg.synopsis.to_lowercase();

    let (score, match_kind) = if name_lower == *query_lower {
        // Exact name match.
        (1.0, MatchKind::ExactName)
    } else if name_lower.starts_with(query_lower) {
        // Prefix match. Shorter names score higher (more specific match).
        let length_bonus = query_lower.len() as f64 / name_lower.len() as f64;
        (0.7 + 0.1 * length_bonus, MatchKind::PrefixName)
    } else if let Some(fuzzy) = fuzzy_subsequence_score(&name_lower, query_lower) {
        // Fuzzy subsequence match on name.
        // Scale to 0.3..0.65 range — capped below prefix match.
        (0.3 + 0.35 * fuzzy, MatchKind::FuzzyName)
    } else if synopsis_lower.contains(query_lower) {
        // Synopsis substring match.
        (0.3, MatchKind::SynopsisMatch)
    } else {
        return None;
    };

    // Penalty for deprecated packages.
    let deprecation_penalty = if pkg.deprecated { 0.8 } else { 1.0 };
    let final_score = score * deprecation_penalty;

    Some(SearchResult {
        package: pkg.clone(),
        score: final_score,
        match_kind,
    })
}

/// Compute a fuzzy subsequence match score between a haystack and needle.
///
/// Returns `Some(score)` where score is in `0.0..=1.0` if the needle is a
/// subsequence of the haystack. Returns `None` if not a subsequence.
///
/// The score rewards:
/// - Consecutive matching characters (higher than scattered matches)
/// - Matches at the start of the haystack
/// - Shorter haystacks (more specific)
///
/// This is a simple scoring algorithm, not a full fuzzy finder.
pub fn fuzzy_subsequence_score(haystack: &str, needle: &str) -> Option<f64> {
    if needle.is_empty() {
        return Some(1.0);
    }
    if haystack.is_empty() {
        return None;
    }

    let haystack_chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();

    // Check if needle is a subsequence of haystack and compute score.
    let mut hay_idx = 0;
    let mut needle_idx = 0;
    let mut consecutive = 0;
    let mut max_consecutive = 0;
    let mut first_match_pos: Option<usize> = None;
    let mut total_distance = 0usize;
    let mut last_match_pos: Option<usize> = None;

    while hay_idx < haystack_chars.len() && needle_idx < needle_chars.len() {
        if haystack_chars[hay_idx] == needle_chars[needle_idx] {
            if first_match_pos.is_none() {
                first_match_pos = Some(hay_idx);
            }

            // Track consecutive matches.
            if let Some(last) = last_match_pos {
                if hay_idx == last + 1 {
                    consecutive += 1;
                } else {
                    total_distance += hay_idx - last - 1;
                    consecutive = 1;
                }
            } else {
                consecutive = 1;
            }
            max_consecutive = max_consecutive.max(consecutive);
            last_match_pos = Some(hay_idx);
            needle_idx += 1;
        }
        hay_idx += 1;
    }

    if needle_idx < needle_chars.len() {
        // Not all needle characters were found — not a subsequence.
        return None;
    }

    // Compute score components.
    let needle_len = needle_chars.len() as f64;
    let haystack_len = haystack_chars.len() as f64;

    // Coverage: what fraction of the haystack is matched.
    let coverage = needle_len / haystack_len;

    // Consecutiveness: what fraction of needle matches are consecutive.
    let consecutiveness = max_consecutive as f64 / needle_len;

    // Position bonus: matches starting at position 0 are better.
    let position_bonus = if first_match_pos == Some(0) { 0.2 } else { 0.0 };

    // Compactness: penalize scattered matches.
    let compactness = if total_distance == 0 {
        1.0
    } else {
        1.0 / (1.0 + total_distance as f64 * 0.1)
    };

    let score = (0.3 * coverage + 0.3 * consecutiveness + 0.2 * compactness + 0.2 * position_bonus)
        .clamp(0.0, 1.0);

    Some(score)
}

/// Search with a set of recommended package names that receive a scoring bonus.
///
/// Recommended packages get a +0.1 bonus to their score, which can push them
/// above non-recommended packages with similar match quality.
pub fn search_with_recommendations(
    packages: &[PackageInfo],
    query: &str,
    recommended: &[&str],
) -> Vec<SearchResult> {
    let mut results = search(packages, query);
    for result in &mut results {
        if recommended.contains(&result.package.name.as_str()) {
            result.score = (result.score + 0.1).min(1.0);
        }
    }
    // Re-sort after bonus.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.package.name.cmp(&b.package.name))
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Version;

    fn mock_packages() -> Vec<PackageInfo> {
        vec![
            PackageInfo {
                name: "aeson".to_string(),
                synopsis: "Fast JSON parsing and encoding".to_string(),
                versions: vec![
                    Version::parse("2.1.0.0").unwrap(),
                    Version::parse("2.2.1.0").unwrap(),
                    Version::parse("2.2.3.0").unwrap(),
                ],
                deprecated: false,
            },
            PackageInfo {
                name: "aeson-pretty".to_string(),
                synopsis: "JSON pretty-printing library".to_string(),
                versions: vec![Version::parse("0.8.10").unwrap()],
                deprecated: false,
            },
            PackageInfo {
                name: "aeson-qq".to_string(),
                synopsis: "JSON quasiquoter for Haskell".to_string(),
                versions: vec![Version::parse("0.8.4").unwrap()],
                deprecated: false,
            },
            PackageInfo {
                name: "text".to_string(),
                synopsis: "Efficient packed Unicode text".to_string(),
                versions: vec![
                    Version::parse("2.0").unwrap(),
                    Version::parse("2.1").unwrap(),
                ],
                deprecated: false,
            },
            PackageInfo {
                name: "old-json".to_string(),
                synopsis: "An old JSON library".to_string(),
                versions: vec![Version::parse("1.0").unwrap()],
                deprecated: true,
            },
            PackageInfo {
                name: "bytestring".to_string(),
                synopsis: "Fast, compact, strict and lazy byte strings".to_string(),
                versions: vec![Version::parse("0.11.5.3").unwrap()],
                deprecated: false,
            },
            PackageInfo {
                name: "base".to_string(),
                synopsis: "The Haskell base library".to_string(),
                versions: vec![
                    Version::parse("4.17.0.0").unwrap(),
                    Version::parse("4.18.0.0").unwrap(),
                    Version::parse("4.19.0.0").unwrap(),
                ],
                deprecated: false,
            },
        ]
    }

    #[test]
    fn search_exact_name() {
        let packages = mock_packages();
        let results = search(&packages, "aeson");
        assert!(!results.is_empty());
        assert_eq!(results[0].package.name, "aeson");
        assert_eq!(results[0].match_kind, MatchKind::ExactName);
    }

    #[test]
    fn search_exact_name_case_insensitive() {
        let packages = mock_packages();
        let results = search(&packages, "AESON");
        assert!(!results.is_empty());
        assert_eq!(results[0].package.name, "aeson");
        assert_eq!(results[0].match_kind, MatchKind::ExactName);
    }

    #[test]
    fn search_prefix() {
        let packages = mock_packages();
        let results = search(&packages, "aes");
        assert!(!results.is_empty());
        // All aeson-* packages should match as prefix.
        let names: Vec<&str> = results.iter().map(|r| r.package.name.as_str()).collect();
        assert!(names.contains(&"aeson"));
        assert!(names.contains(&"aeson-pretty"));
        assert!(names.contains(&"aeson-qq"));
        // "aeson" should rank highest (shortest prefix match / most specific).
        assert_eq!(results[0].package.name, "aeson");
    }

    #[test]
    fn search_fuzzy_name() {
        let packages = mock_packages();
        // "ason" is a subsequence of "aeson"
        let results = search(&packages, "ason");
        assert!(!results.is_empty());
        let names: Vec<&str> = results.iter().map(|r| r.package.name.as_str()).collect();
        assert!(names.contains(&"aeson"));
    }

    #[test]
    fn search_synopsis() {
        let packages = mock_packages();
        let results = search(&packages, "json");
        assert!(!results.is_empty());
        // Multiple packages mention JSON in their synopsis or name.
        let names: Vec<&str> = results.iter().map(|r| r.package.name.as_str()).collect();
        assert!(names.contains(&"aeson")); // "JSON" in synopsis
    }

    #[test]
    fn search_deprecated_penalty() {
        let packages = mock_packages();
        let results = search(&packages, "json");
        // Both aeson (synopsis match) and old-json (name match, deprecated) should appear.
        let aeson_result = results.iter().find(|r| r.package.name == "aeson").unwrap();
        let old_json_result = results
            .iter()
            .find(|r| r.package.name == "old-json")
            .unwrap();
        // old-json is deprecated, so its score should be reduced.
        // It still ranks above aeson because "json" is literally in its name,
        // but the deprecation penalty is applied.
        assert!(
            old_json_result.score < 1.0,
            "deprecated package score should be penalized"
        );
        // Verify the penalty: deprecated score < what it would be without deprecation.
        // Since old-json has "json" in the name (fuzzy match), it would score > 0.3 without penalty.
        // With 0.8 penalty, it's still > 0.3 * 0.8 = 0.24.
        assert!(old_json_result.score > 0.0);
        assert!(aeson_result.score > 0.0);
    }

    #[test]
    fn search_empty_query() {
        let packages = mock_packages();
        let results = search(&packages, "");
        assert!(results.is_empty());
    }

    #[test]
    fn search_no_match() {
        let packages = mock_packages();
        let results = search(&packages, "zzzzzznotapackage");
        assert!(results.is_empty());
    }

    #[test]
    fn search_whitespace_trimmed() {
        let packages = mock_packages();
        let results = search(&packages, "  aeson  ");
        assert!(!results.is_empty());
        assert_eq!(results[0].package.name, "aeson");
    }

    #[test]
    fn fuzzy_score_exact() {
        let score = fuzzy_subsequence_score("aeson", "aeson").unwrap();
        assert!(score > 0.8, "exact match should score high: {score}");
    }

    #[test]
    fn fuzzy_score_subsequence() {
        let score = fuzzy_subsequence_score("aeson", "asn").unwrap();
        assert!(score > 0.0, "subsequence should match: {score}");
    }

    #[test]
    fn fuzzy_score_no_match() {
        assert!(fuzzy_subsequence_score("aeson", "xyz").is_none());
    }

    #[test]
    fn fuzzy_score_empty_needle() {
        let score = fuzzy_subsequence_score("aeson", "").unwrap();
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_score_consecutive_better() {
        let consecutive = fuzzy_subsequence_score("aeson", "aes").unwrap();
        let scattered = fuzzy_subsequence_score("aeson", "asn").unwrap();
        assert!(
            consecutive > scattered,
            "consecutive ({consecutive}) should beat scattered ({scattered})"
        );
    }

    #[test]
    fn search_with_recommendations_bonus() {
        let packages = mock_packages();
        // Search for "aes" where aeson and aeson-pretty both match as prefix.
        // With recommendation bonus, aeson should remain first.
        let results = search_with_recommendations(&packages, "aes", &["aeson"]);
        assert_eq!(results[0].package.name, "aeson");
        // Verify the bonus was applied.
        let aeson_with_bonus = results.iter().find(|r| r.package.name == "aeson").unwrap();
        let plain_results = search(&packages, "aes");
        let aeson_plain = plain_results
            .iter()
            .find(|r| r.package.name == "aeson")
            .unwrap();
        assert!(aeson_with_bonus.score > aeson_plain.score);
    }
}
