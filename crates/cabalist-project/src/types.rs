//! Types representing a parsed `cabal.project` file.

use serde::{Deserialize, Serialize};

/// A parsed `cabal.project` file.
///
/// This captures the key fields from a `cabal.project` (or `cabal.project.local`,
/// `cabal.project.freeze`) file in a structured form. Round-trip fidelity is not
/// a goal -- this is a read-only representation for querying project configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CabalProject {
    /// Raw source text.
    pub source: String,
    /// Packages to include (glob patterns from the `packages:` field).
    pub packages: Vec<String>,
    /// Optional packages (from the `optional-packages:` field).
    pub optional_packages: Vec<String>,
    /// Extra packages to include (from the `extra-packages:` field).
    pub extra_packages: Vec<String>,
    /// Compiler to use (from the `with-compiler:` field).
    pub with_compiler: Option<String>,
    /// Index state timestamp (from the `index-state:` field).
    pub index_state: Option<String>,
    /// Global constraints (from the `constraints:` field, split on commas).
    pub constraints: Vec<String>,
    /// Allow-newer constraints (from the `allow-newer:` field, split on commas).
    pub allow_newer: Vec<String>,
    /// Allow-older constraints (from the `allow-older:` field, split on commas).
    pub allow_older: Vec<String>,
    /// Per-package stanzas (`package <name>` or `package *`).
    pub package_stanzas: Vec<PackageStanza>,
    /// Source repository packages (`source-repository-package` stanzas).
    pub source_repo_packages: Vec<SourceRepoPackage>,
    /// All other top-level fields not captured above.
    pub other_fields: Vec<(String, String)>,
}

/// A `package <name>` stanza within a `cabal.project` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageStanza {
    /// Package name, or `"*"` for the global package stanza.
    pub name: String,
    /// Fields within the stanza as `(field-name, value)` pairs.
    pub fields: Vec<(String, String)>,
}

/// A `source-repository-package` stanza.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceRepoPackage {
    /// Repository type (e.g., `"git"`, `"mercurial"`).
    pub repo_type: Option<String>,
    /// Repository location URL.
    pub location: Option<String>,
    /// Tag to check out.
    pub tag: Option<String>,
    /// Branch to check out.
    pub branch: Option<String>,
    /// Subdirectory within the repository.
    pub subdir: Option<String>,
}
