//! User configuration from `cabalist.toml`.
//!
//! Configuration is loaded from the project root first, then from the user's
//! config directory (`~/.config/cabalist/config.toml`). Project-level settings
//! override user-level settings, which override built-in defaults.

use crate::defaults;
use crate::lints::LintConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Error type for configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Failed to read the configuration file.
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    /// Failed to parse the TOML configuration.
    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
}

/// The full cabalist configuration (from `cabalist.toml`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CabalistConfig {
    /// Default values for new projects and components.
    pub defaults: DefaultsConfig,
    /// Lint configuration.
    pub lints: LintsConfig,
    /// Formatting preferences.
    pub formatting: FormattingConfig,
}

/// Default values configuration section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    /// Default `cabal-version` for new projects.
    pub cabal_version: String,
    /// Default language for new components.
    pub default_language: String,
    /// Default license for new projects.
    pub license: String,
    /// Override GHC options.
    pub ghc_options: Option<GhcOptionsConfig>,
    /// Override default extensions.
    pub extensions: Option<ExtensionsConfig>,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            cabal_version: defaults::DEFAULT_CABAL_VERSION.to_string(),
            default_language: defaults::DEFAULT_LANGUAGE.to_string(),
            license: defaults::DEFAULT_LICENSE.to_string(),
            ghc_options: None,
            extensions: None,
        }
    }
}

impl DefaultsConfig {
    /// Return the effective GHC options: the user override if set, otherwise
    /// the built-in defaults.
    pub fn effective_ghc_options(&self) -> Vec<String> {
        match &self.ghc_options {
            Some(config) => config.options.clone(),
            None => defaults::DEFAULT_GHC_OPTIONS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Return the effective default extensions: the user override if set,
    /// otherwise the built-in defaults.
    pub fn effective_extensions(&self) -> Vec<String> {
        match &self.extensions {
            Some(config) => config.extensions.clone(),
            None => defaults::DEFAULT_EXTENSIONS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// GHC options override configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct GhcOptionsConfig {
    /// The GHC options to use (replaces the built-in defaults entirely).
    pub options: Vec<String>,
}

/// Extensions override configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionsConfig {
    /// The extensions to use (replaces the built-in defaults entirely).
    pub extensions: Vec<String>,
}

/// Lint configuration section.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LintsConfig {
    /// Lint IDs to disable.
    pub disable: Vec<String>,
    /// Lint IDs to promote to error severity.
    #[serde(default)]
    pub error: Vec<String>,
}

impl LintsConfig {
    /// Convert to the `LintConfig` type used by the lints module.
    pub fn to_lint_config(&self) -> LintConfig {
        LintConfig {
            disabled: self.disable.clone(),
            errors: self.error.clone(),
        }
    }
}

/// Formatting preferences section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FormattingConfig {
    /// Sort dependencies alphabetically.
    pub sort_dependencies: bool,
    /// Sort module lists alphabetically.
    pub sort_modules: bool,
    /// Indentation width in spaces.
    pub indent: usize,
}

impl Default for FormattingConfig {
    fn default() -> Self {
        Self {
            sort_dependencies: true,
            sort_modules: true,
            indent: 2,
        }
    }
}

/// Load configuration from a TOML file at the given path.
pub fn load_config(path: &Path) -> Result<CabalistConfig, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: CabalistConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Find and load configuration, searching the project root first, then the
/// user config directory.
///
/// Returns the built-in defaults if no configuration file is found.
pub fn find_and_load_config(project_root: &Path) -> CabalistConfig {
    // 1. Check project root.
    let project_config = project_root.join("cabalist.toml");
    if project_config.exists() {
        if let Ok(config) = load_config(&project_config) {
            return config;
        }
    }

    // 2. Check user config directory.
    if let Some(config_dir) = user_config_dir() {
        let user_config = config_dir.join("config.toml");
        if user_config.exists() {
            if let Ok(config) = load_config(&user_config) {
                return config;
            }
        }
    }

    // 3. Return defaults.
    CabalistConfig::default()
}

/// Return the user configuration directory (`~/.config/cabalist/`).
fn user_config_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .map(|p| p.join("cabalist"))
    }
    #[cfg(not(unix))]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("cabalist"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabalist_parser::diagnostic::Severity;

    #[test]
    fn parse_minimal_config() {
        let toml = "";
        let config: CabalistConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.defaults.cabal_version,
            defaults::DEFAULT_CABAL_VERSION
        );
        assert_eq!(config.defaults.license, defaults::DEFAULT_LICENSE);
    }

    #[test]
    fn parse_config_with_overrides() {
        let toml = r#"
[defaults]
cabal_version = "2.4"
default_language = "Haskell2010"
license = "BSD-3-Clause"

[defaults.ghc_options]
options = ["-Wall", "-Werror"]

[defaults.extensions]
extensions = ["OverloadedStrings", "StrictData"]

[lints]
disable = ["missing-description"]
error = ["missing-upper-bound"]

[formatting]
sort_dependencies = false
indent = 4
"#;
        let config: CabalistConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.defaults.cabal_version, "2.4");
        assert_eq!(config.defaults.default_language, "Haskell2010");
        assert_eq!(config.defaults.license, "BSD-3-Clause");

        let ghc_opts = config.defaults.effective_ghc_options();
        assert_eq!(ghc_opts, vec!["-Wall", "-Werror"]);

        let exts = config.defaults.effective_extensions();
        assert_eq!(exts, vec!["OverloadedStrings", "StrictData"]);

        assert_eq!(config.lints.disable, vec!["missing-description"]);
        assert_eq!(config.lints.error, vec!["missing-upper-bound"]);

        assert!(!config.formatting.sort_dependencies);
        assert_eq!(config.formatting.indent, 4);
    }

    #[test]
    fn effective_ghc_options_defaults() {
        let config = DefaultsConfig::default();
        let opts = config.effective_ghc_options();
        assert_eq!(opts.len(), defaults::DEFAULT_GHC_OPTIONS.len());
    }

    #[test]
    fn effective_extensions_defaults() {
        let config = DefaultsConfig::default();
        let exts = config.effective_extensions();
        assert_eq!(exts.len(), defaults::DEFAULT_EXTENSIONS.len());
    }

    #[test]
    fn lint_config_conversion() {
        let lints_config = LintsConfig {
            disable: vec!["foo".to_string()],
            error: vec!["bar".to_string()],
        };
        let lint_config = lints_config.to_lint_config();
        assert!(!lint_config.is_enabled("foo"));
        assert!(lint_config.is_enabled("bar"));
        assert_eq!(
            lint_config.effective_severity("bar", Severity::Warning),
            Severity::Error
        );
    }

    #[test]
    fn find_config_returns_defaults_for_nonexistent_path() {
        let config = find_and_load_config(Path::new("/nonexistent/path/that/does/not/exist"));
        assert_eq!(
            config.defaults.cabal_version,
            defaults::DEFAULT_CABAL_VERSION
        );
    }

    #[test]
    fn formatting_defaults() {
        let fmt = FormattingConfig::default();
        assert!(fmt.sort_dependencies);
        assert!(fmt.sort_modules);
        assert_eq!(fmt.indent, 2);
    }
}
