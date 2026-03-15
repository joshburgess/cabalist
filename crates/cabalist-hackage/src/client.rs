//! HTTP client for downloading and refreshing the Hackage package index.
//!
//! This module is only available when the `network` feature is enabled.
//! It handles downloading the Hackage `01-index.tar.gz`, extracting package
//! metadata, and building a [`HackageIndex`].

#![cfg(feature = "network")]

use crate::error::HackageError;
use crate::index::HackageIndex;
use crate::types::{PackageInfo, Version};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The URL of the Hackage package index tarball.
const INDEX_URL: &str = "https://hackage.haskell.org/01-index.tar.gz";

/// Filename for the downloaded compressed index.
const INDEX_FILENAME: &str = "01-index.tar.gz";

/// Filename for the timestamp of the last successful download.
const TIMESTAMP_FILENAME: &str = "01-index.timestamp";

/// Filename for the pre-processed JSON index cache.
const CACHE_FILENAME: &str = "index.json";

/// Download or update the Hackage package index.
///
/// This function:
/// 1. Checks if a cached index exists and is fresh (using HTTP `If-Modified-Since`).
/// 2. If stale or missing, downloads the full `01-index.tar.gz` (~150MB compressed).
/// 3. Extracts package metadata from the tarball.
/// 4. Saves a pre-processed JSON cache for fast subsequent loads.
///
/// Returns the loaded index.
pub async fn update_index(cache_dir: &Path) -> Result<HackageIndex, HackageError> {
    std::fs::create_dir_all(cache_dir)?;

    let index_path = cache_dir.join(INDEX_FILENAME);
    let timestamp_path = cache_dir.join(TIMESTAMP_FILENAME);
    let cache_path = cache_dir.join(CACHE_FILENAME);

    // Check if we need to download.
    let needs_download = if index_path.exists() {
        check_index_freshness(&timestamp_path).await?
    } else {
        true
    };

    if needs_download {
        download_index_file(&index_path, &timestamp_path).await?;
        let index = parse_index_tarball(&index_path)?;
        index.save_to_cache(&cache_path)?;
        Ok(index)
    } else if cache_path.exists() {
        // Use the existing pre-processed cache.
        HackageIndex::load_from_cache(&cache_path)
    } else {
        // Tarball exists but cache doesn't — re-parse.
        let index = parse_index_tarball(&index_path)?;
        index.save_to_cache(&cache_path)?;
        Ok(index)
    }
}

/// Check if the remote index has been modified since our last download.
///
/// Returns `true` if the index needs to be re-downloaded.
async fn check_index_freshness(timestamp_path: &Path) -> Result<bool, HackageError> {
    let last_modified = match std::fs::read_to_string(timestamp_path) {
        Ok(ts) => ts.trim().to_string(),
        Err(_) => return Ok(true), // No timestamp — need download.
    };

    let client = reqwest::Client::new();
    let response = client
        .head(INDEX_URL)
        .header("If-Modified-Since", &last_modified)
        .send()
        .await?;

    // 304 Not Modified means our cache is fresh.
    Ok(response.status() != reqwest::StatusCode::NOT_MODIFIED)
}

/// Download the index tarball from Hackage.
async fn download_index_file(index_path: &Path, timestamp_path: &Path) -> Result<(), HackageError> {
    let client = reqwest::Client::new();
    let response = client.get(INDEX_URL).send().await?;

    // Save the Last-Modified header for future conditional requests.
    if let Some(last_modified) = response.headers().get("Last-Modified") {
        if let Ok(value) = last_modified.to_str() {
            std::fs::write(timestamp_path, value)?;
        }
    }

    let bytes = response.bytes().await?;
    std::fs::write(index_path, &bytes)?;

    Ok(())
}

/// Parse the `01-index.tar.gz` tarball and extract package metadata.
///
/// The tarball contains one `.cabal` file per package version, organized as:
/// `<package-name>/<version>/<package-name>.cabal`
///
/// We extract the package name, version, and synopsis from each entry.
fn parse_index_tarball(tarball_path: &Path) -> Result<HackageIndex, HackageError> {
    let file = std::fs::File::open(tarball_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    // Collect package name -> (versions, synopsis).
    let mut packages: HashMap<String, (Vec<Version>, String)> = HashMap::new();

    for entry in archive.entries()? {
        let mut entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed entries.
        };

        let path = match entry.path() {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };

        // We only care about .cabal files.
        let path_str = path.to_string_lossy().to_string();
        if !path_str.ends_with(".cabal") {
            continue;
        }

        // Parse path: <name>/<version>/<name>.cabal
        let parts: Vec<&str> = path_str.split('/').collect();
        if parts.len() != 3 {
            continue;
        }

        let pkg_name = parts[0].to_string();
        let version_str = parts[1];

        let version = match Version::parse(version_str) {
            Some(v) => v,
            None => continue,
        };

        let entry_data = packages
            .entry(pkg_name)
            .or_insert_with(|| (Vec::new(), String::new()));

        entry_data.0.push(version);

        // Only parse synopsis from the latest entry we see (the last version
        // in the tarball is typically the latest).
        if entry_data.1.is_empty() {
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
                if let Some(synopsis) = extract_synopsis(&content) {
                    entry_data.1 = synopsis;
                }
            }
        }
    }

    let package_list: Vec<PackageInfo> = packages
        .into_iter()
        .map(|(name, (mut versions, synopsis))| {
            versions.sort();
            PackageInfo {
                name,
                synopsis,
                versions,
                deprecated: false, // TODO: check preferred-versions
            }
        })
        .collect();

    Ok(HackageIndex::from_packages(package_list))
}

/// Extract the `synopsis` field from a `.cabal` file's raw text.
///
/// This is a simple line-based extraction — we don't use the full parser
/// here to avoid a circular dependency.
fn extract_synopsis(cabal_content: &str) -> Option<String> {
    for line in cabal_content.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("synopsis:") {
            let value = trimmed["synopsis:".len()..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Get the default paths for cache files.
pub fn cache_paths(cache_dir: &Path) -> CachePaths {
    CachePaths {
        index_tarball: cache_dir.join(INDEX_FILENAME),
        timestamp: cache_dir.join(TIMESTAMP_FILENAME),
        json_cache: cache_dir.join(CACHE_FILENAME),
    }
}

/// Paths to the various cache files.
pub struct CachePaths {
    /// The downloaded `01-index.tar.gz`.
    pub index_tarball: PathBuf,
    /// The timestamp file for conditional HTTP requests.
    pub timestamp: PathBuf,
    /// The pre-processed JSON cache.
    pub json_cache: PathBuf,
}
