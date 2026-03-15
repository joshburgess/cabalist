//! Error types for cabal CLI operations.

use std::time::Duration;

/// Errors that can occur when interacting with the `cabal` CLI.
#[derive(Debug, thiserror::Error)]
pub enum CabalError {
    /// An IO error occurred (e.g., spawning subprocess).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse JSON (e.g., plan.json).
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// The `cabal` command exited with a non-zero exit code.
    #[error("cabal command failed with exit code {exit_code}: {stderr}")]
    CommandFailed {
        /// The process exit code.
        exit_code: i32,
        /// Captured stderr output.
        stderr: String,
    },

    /// The `cabal` executable was not found on `PATH`.
    #[error("cabal not found on PATH")]
    CabalNotFound,

    /// The `ghc` executable was not found on `PATH`.
    #[error("ghc not found on PATH")]
    GhcNotFound,

    /// The command exceeded its configured timeout.
    #[error("command timed out after {0:?}")]
    Timeout(Duration),

    /// The command was cancelled (e.g., the output receiver was dropped).
    #[error("command cancelled")]
    Cancelled,
}
