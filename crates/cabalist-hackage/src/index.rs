//! Hackage package index management.
//!
//! Provides loading, caching, and querying of the Hackage package index.
//! The network-dependent operations (downloading the full index tarball)
//! are gated behind the `network` feature flag.

use crate::error::HackageError;
use crate::search::{self, SearchResult};
use crate::types::{PackageInfo, Version};
use std::collections::HashMap;
use std::path::Path;

/// The Hackage package index, loaded into memory for fast querying.
#[derive(Debug, Clone)]
pub struct HackageIndex {
    /// All packages, keyed by name for O(1) lookup.
    packages_by_name: HashMap<String, PackageInfo>,
    /// All packages as a flat list (for search).
    packages: Vec<PackageInfo>,
}

impl HackageIndex {
    /// Create an empty index.
    pub fn empty() -> Self {
        Self {
            packages_by_name: HashMap::new(),
            packages: Vec::new(),
        }
    }

    /// Create an index from a list of packages.
    pub fn from_packages(packages: Vec<PackageInfo>) -> Self {
        let packages_by_name: HashMap<String, PackageInfo> = packages
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();
        Self {
            packages_by_name,
            packages,
        }
    }

    /// Load the index from a pre-processed JSON cache file.
    ///
    /// The cache file is a JSON array of [`PackageInfo`] objects, written
    /// by [`save_to_cache`](Self::save_to_cache).
    pub fn load_from_cache(path: &Path) -> Result<Self, HackageError> {
        if !path.exists() {
            return Err(HackageError::IndexNotFound(path.to_path_buf()));
        }
        let data = std::fs::read_to_string(path)?;
        let packages: Vec<PackageInfo> = serde_json::from_str(&data)?;
        Ok(Self::from_packages(packages))
    }

    /// Save the index to a JSON cache file for fast reloading.
    pub fn save_to_cache(&self, path: &Path) -> Result<(), HackageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string(&self.packages)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Search the index by query string.
    ///
    /// Returns results sorted by relevance. See [`search::search`] for
    /// ranking details.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        search::search(&self.packages, query)
    }

    /// Look up a specific package by exact name.
    pub fn package_info(&self, name: &str) -> Option<&PackageInfo> {
        self.packages_by_name.get(name)
    }

    /// Get the latest version of a package.
    pub fn latest_version(&self, name: &str) -> Option<&Version> {
        self.package_info(name).and_then(|pkg| pkg.latest_version())
    }

    /// Get all versions of a package.
    pub fn package_versions(&self, name: &str) -> Option<&[Version]> {
        self.package_info(name).map(|pkg| pkg.versions.as_slice())
    }

    /// Return the total number of packages in the index.
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Return true if the index contains no packages.
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Return an iterator over all packages.
    pub fn iter(&self) -> impl Iterator<Item = &PackageInfo> {
        self.packages.iter()
    }

    /// Get the default cache directory for the Hackage index.
    ///
    /// Returns `~/.cache/cabalist/` on Linux/macOS, or the platform-appropriate
    /// cache directory.
    #[cfg(feature = "network")]
    pub fn default_cache_dir() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("", "", "cabalist")
            .map(|dirs| dirs.cache_dir().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_packages() -> Vec<PackageInfo> {
        vec![
            PackageInfo {
                name: "aeson".to_string(),
                synopsis: "Fast JSON parsing and encoding".to_string(),
                versions: vec![
                    Version::parse("2.1.0.0").unwrap(),
                    Version::parse("2.2.3.0").unwrap(),
                ],
                deprecated: false,
            },
            PackageInfo {
                name: "text".to_string(),
                synopsis: "Efficient packed Unicode text".to_string(),
                versions: vec![Version::parse("2.1").unwrap()],
                deprecated: false,
            },
        ]
    }

    #[test]
    fn empty_index() {
        let idx = HackageIndex::empty();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert!(idx.package_info("aeson").is_none());
    }

    #[test]
    fn from_packages() {
        let idx = HackageIndex::from_packages(sample_packages());
        assert_eq!(idx.len(), 2);
        assert!(idx.package_info("aeson").is_some());
        assert!(idx.package_info("text").is_some());
        assert!(idx.package_info("nonexistent").is_none());
    }

    #[test]
    fn latest_version_lookup() {
        let idx = HackageIndex::from_packages(sample_packages());
        let v = idx.latest_version("aeson").unwrap();
        assert_eq!(v, &Version::parse("2.2.3.0").unwrap());
    }

    #[test]
    fn package_versions_lookup() {
        let idx = HackageIndex::from_packages(sample_packages());
        let versions = idx.package_versions("aeson").unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn search_on_index() {
        let idx = HackageIndex::from_packages(sample_packages());
        let results = idx.search("aeson");
        assert!(!results.is_empty());
        assert_eq!(results[0].package.name, "aeson");
    }

    #[test]
    fn cache_round_trip() {
        let idx = HackageIndex::from_packages(sample_packages());
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("index.json");

        idx.save_to_cache(&cache_path).unwrap();
        let loaded = HackageIndex::load_from_cache(&cache_path).unwrap();

        assert_eq!(loaded.len(), idx.len());
        assert_eq!(
            loaded.package_info("aeson").unwrap().name,
            idx.package_info("aeson").unwrap().name
        );
        assert_eq!(loaded.latest_version("aeson"), idx.latest_version("aeson"));
    }

    #[test]
    fn load_nonexistent_cache() {
        let result = HackageIndex::load_from_cache(Path::new("/nonexistent/path/index.json"));
        assert!(result.is_err());
    }

    #[test]
    fn iterator() {
        let idx = HackageIndex::from_packages(sample_packages());
        let names: Vec<&str> = idx.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"aeson"));
        assert!(names.contains(&"text"));
    }
}
