//! # cabalist-hackage
//!
//! Interface to the Hackage package index. Supports downloading and caching the
//! package index, fuzzy searching packages by name and synopsis, fetching version
//! information, and computing PVP-compliant version bounds.
//!
//! ## Feature Flags
//!
//! - `network` — Enables downloading the Hackage index over HTTP. Pulls in
//!   `reqwest`, `flate2`, `tar`, and `directories`. Without this feature,
//!   only the pure logic (PVP computation, search, cache loading) is available.
//!
//! ## Quick Start
//!
//! ```rust
//! use cabalist_hackage::{Version, compute_pvp_bounds};
//!
//! let version = Version::parse("2.2.1.0").unwrap();
//! let bounds = compute_pvp_bounds(&version);
//! assert_eq!(bounds.to_string(), ">=2.2.1.0 && <2.3");
//! ```

/// Error types for Hackage operations.
pub mod error;
/// In-memory package index with cache persistence.
pub mod index;
/// PVP version bound computation.
pub mod pvp;
/// Fuzzy package search and ranking.
pub mod search;
/// Core types: `Version`, `PackageInfo`, `VersionRange`.
pub mod types;

/// HTTP client for downloading and updating the Hackage index.
#[cfg(feature = "network")]
pub mod client;

// Re-export key types at crate root for convenience.
pub use error::HackageError;
pub use index::HackageIndex;
pub use pvp::{compute_major_bound, compute_pvp_bounds, suggest_bounds, version_satisfies};
pub use search::{search_with_recommendations, MatchKind, SearchResult};
pub use types::{PackageInfo, Version, VersionRange};

/// Search the given package list. This is a convenience wrapper for
/// [`search::search`].
pub fn search_packages(packages: &[PackageInfo], query: &str) -> Vec<SearchResult> {
    search::search(packages, query)
}
