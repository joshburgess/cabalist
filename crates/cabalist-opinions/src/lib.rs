//! # cabalist-opinions
//!
//! Encodes opinionated best practices for Haskell `.cabal` files as a reusable
//! library. Provides structured lints, suggested defaults, project templates,
//! and user-configurable overrides via `cabalist.toml`. Every opinion is
//! documented with rationale and individually disableable.
//!
//! ## Modules
//!
//! - [`defaults`] — Opinionated default values for new projects (cabal-version,
//!   language, GHC options, extensions, directory layout).
//! - [`deps`] — Curated database of recommended packages for common tasks.
//! - [`lints`] — Opinionated lints that check `.cabal` files against best
//!   practices. Each lint has a unique ID and can be disabled via config.
//! - [`templates`] — Project templates for `cabalist init`.
//! - [`config`] — User configuration loading from `cabalist.toml`.

pub mod config;
pub mod defaults;
pub mod deps;
pub mod lints;
pub mod templates;

// Re-export key types at the crate root for convenience.
pub use config::{CabalistConfig, ConfigError};
pub use defaults::{
    DEFAULT_CABAL_VERSION, DEFAULT_EXTENSIONS, DEFAULT_GHC_OPTIONS, DEFAULT_LANGUAGE,
    DEFAULT_LAYOUT, DEFAULT_LICENSE,
};
pub use lints::{run_lints, Lint, LintConfig, ALL_LINT_IDS};
pub use templates::{render_template, TemplateKind, TemplateVars};
