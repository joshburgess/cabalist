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

/// User configuration loading from `cabalist.toml`.
pub mod config;
/// Opinionated default values for new projects.
pub mod defaults;
/// Curated database of recommended packages for common tasks.
pub mod deps;
/// `.cabal` file formatter.
pub mod fmt;
/// Opinionated lints that check `.cabal` files against best practices.
pub mod lints;
/// Project templates for `cabalist init`.
pub mod templates;

pub use config::{CabalistConfig, ConfigError};
pub use defaults::{
    DEFAULT_CABAL_VERSION, DEFAULT_EXTENSIONS, DEFAULT_GHC_OPTIONS, DEFAULT_LANGUAGE,
    DEFAULT_LAYOUT, DEFAULT_LICENSE,
};
pub use lints::{
    run_all_lints, run_all_lints_with_cst, run_fs_lints, run_fs_lints_with_cst, run_lints,
    run_lints_with_cst, Lint, LintConfig, ALL_LINT_IDS,
};
pub use templates::{render_template, TemplateKind, TemplateVars};
