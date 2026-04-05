//! # cabalist-ghc
//!
//! A static knowledge base about GHC extensions, warnings, and versions.
//! Provides queryable databases for extension metadata (description, since
//! which GHC version, safety, recommendation status) and warning flags
//! (groups, recommendations). Also maps GHC versions to `base` library versions.

/// GHC language extension database with metadata.
pub mod extensions;
/// Extensions included in the GHC2021 language edition.
pub mod ghc2021;
/// GHC version to `base` library version mappings.
pub mod versions;
/// GHC warning flags and groups.
pub mod warnings;

pub use extensions::Extension;
pub use versions::GhcBaseMapping;
pub use warnings::Warning;
