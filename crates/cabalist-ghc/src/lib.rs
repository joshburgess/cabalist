//! # cabalist-ghc
//!
//! A static knowledge base about GHC extensions, warnings, and versions.
//! Provides queryable databases for extension metadata (description, since
//! which GHC version, safety, recommendation status) and warning flags
//! (groups, recommendations). Also maps GHC versions to `base` library versions.

pub mod extensions;
pub mod ghc2021;
pub mod versions;
pub mod warnings;

pub use extensions::Extension;
pub use versions::GhcBaseMapping;
pub use warnings::Warning;
