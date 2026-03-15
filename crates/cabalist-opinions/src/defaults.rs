//! Opinionated default values for new Haskell projects.
//!
//! These defaults represent the project's considered best practices for modern
//! Haskell development. Every default is documented with rationale and can be
//! overridden via `cabalist.toml`.

/// Default `cabal-version` for new projects.
///
/// 3.0 unlocks common stanzas and `import`, which are essential for
/// maintainable `.cabal` files. We do not support `cabal-version < 2.2` for
/// new projects.
pub const DEFAULT_CABAL_VERSION: &str = "3.0";

/// Default language for new components.
///
/// `GHC2021` is preferred when the detected GHC is >= 9.2. Otherwise we fall
/// back to `Haskell2010`.
pub const DEFAULT_LANGUAGE: &str = "GHC2021";

/// Fallback language when GHC < 9.2 is detected.
pub const FALLBACK_LANGUAGE: &str = "Haskell2010";

/// Default license for new projects.
pub const DEFAULT_LICENSE: &str = "MIT";

/// Default GHC warning options.
///
/// This set catches most common mistakes without being so noisy that people
/// disable warnings entirely.
pub const DEFAULT_GHC_OPTIONS: &[&str] = &[
    "-Wall",
    "-Wcompat",
    "-Widentities",
    "-Wincomplete-record-updates",
    "-Wincomplete-uni-patterns",
    "-Wmissing-deriving-strategies",
    "-Wredundant-constraints",
    "-Wunused-packages",
];

/// Default extensions for new projects.
///
/// These are widely considered safe defaults that reduce boilerplate without
/// changing semantics in surprising ways. Notably absent: `StrictData` (too
/// opinionated), `TemplateHaskell` (compile-time cost), `UndecidableInstances`
/// (type-checker footgun).
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "OverloadedStrings",
    "DerivingStrategies",
    "DeriveGeneric",
    "DeriveAnyClass",
    "GeneralizedNewtypeDeriving",
    "LambdaCase",
    "TypeApplications",
    "ScopedTypeVariables",
];

/// Default project directory layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLayout {
    /// Library source directory.
    pub library_src: &'static str,
    /// Executable source directory.
    pub executable_src: &'static str,
    /// Test source directory.
    pub test_src: &'static str,
    /// Benchmark source directory.
    pub benchmark_src: &'static str,
}

/// The recommended directory layout for new projects.
pub const DEFAULT_LAYOUT: ProjectLayout = ProjectLayout {
    library_src: "src",
    executable_src: "app",
    test_src: "test",
    benchmark_src: "bench",
};

/// Returns the appropriate default language for a given GHC version string.
///
/// Returns `GHC2021` for GHC >= 9.2, `Haskell2010` otherwise.
pub fn language_for_ghc_version(ghc_version: &str) -> &'static str {
    if cabalist_ghc::versions::supports_ghc2021(ghc_version) {
        DEFAULT_LANGUAGE
    } else {
        FALLBACK_LANGUAGE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_constants_are_non_empty() {
        assert!(!DEFAULT_CABAL_VERSION.is_empty());
        assert!(!DEFAULT_LANGUAGE.is_empty());
        assert!(!DEFAULT_LICENSE.is_empty());
        assert!(!DEFAULT_GHC_OPTIONS.is_empty());
        assert!(!DEFAULT_EXTENSIONS.is_empty());
    }

    #[test]
    fn default_layout_values() {
        assert_eq!(DEFAULT_LAYOUT.library_src, "src");
        assert_eq!(DEFAULT_LAYOUT.executable_src, "app");
        assert_eq!(DEFAULT_LAYOUT.test_src, "test");
        assert_eq!(DEFAULT_LAYOUT.benchmark_src, "bench");
    }

    #[test]
    fn language_selection() {
        assert_eq!(language_for_ghc_version("9.8.2"), DEFAULT_LANGUAGE);
        assert_eq!(language_for_ghc_version("9.2.1"), DEFAULT_LANGUAGE);
        assert_eq!(language_for_ghc_version("9.0.2"), FALLBACK_LANGUAGE);
        assert_eq!(language_for_ghc_version("8.10.7"), FALLBACK_LANGUAGE);
    }
}
