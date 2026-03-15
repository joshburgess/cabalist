//! Error types for the Hackage interface.

use std::path::PathBuf;

/// Errors that can occur when interacting with the Hackage index.
#[derive(Debug, thiserror::Error)]
pub enum HackageError {
    /// An I/O error occurred (reading/writing cache files, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization/deserialization error occurred.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// An HTTP error occurred while communicating with Hackage.
    #[cfg(feature = "network")]
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The index cache file was not found at the expected path.
    #[error("Index not found at {0}")]
    IndexNotFound(PathBuf),

    /// A package was not found in the index.
    #[error("Package not found: {0}")]
    PackageNotFound(String),
}
