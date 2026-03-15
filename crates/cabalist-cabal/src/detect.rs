//! Detection of Haskell toolchain (cabal, ghc, ghcup) and project setup.

use std::path::Path;
use tokio::process::Command;

/// Information about the detected Haskell development environment.
#[derive(Debug, Clone, Default)]
pub struct ToolchainInfo {
    /// Detected `cabal-install` version, e.g. `"3.10.2.1"`.
    pub cabal_version: Option<String>,
    /// Detected GHC version, e.g. `"9.8.2"`.
    pub ghc_version: Option<String>,
    /// Whether `ghcup` is installed.
    pub ghcup_installed: bool,
    /// Whether a `cabal.project` file exists in the project directory.
    pub has_cabal_project: bool,
    /// Whether a `stack.yaml` file exists in the project directory.
    pub has_stack_yaml: bool,
}

/// Detect the Haskell toolchain and project setup at the given directory.
///
/// Runs tool detection commands concurrently. Never fails; fields that cannot
/// be detected are left as `None` or `false`.
pub async fn detect_toolchain(project_dir: &Path) -> ToolchainInfo {
    let (cabal_version, ghc_version, ghcup_installed) =
        tokio::join!(detect_cabal_version(), detect_ghc_version(), detect_ghcup());

    let has_cabal_project = project_dir.join("cabal.project").is_file();
    let has_stack_yaml = project_dir.join("stack.yaml").is_file();

    ToolchainInfo {
        cabal_version,
        ghc_version,
        ghcup_installed,
        has_cabal_project,
        has_stack_yaml,
    }
}

/// Check if `cabal` is installed and return its version string.
pub async fn detect_cabal_version() -> Option<String> {
    run_version_command("cabal", &["--numeric-version"]).await
}

/// Check if `ghc` is installed and return its version string.
pub async fn detect_ghc_version() -> Option<String> {
    run_version_command("ghc", &["--numeric-version"]).await
}

/// Check if `ghcup` is installed.
pub async fn detect_ghcup() -> bool {
    Command::new("ghcup")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a command that prints a version string and return it trimmed.
async fn run_version_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolchain_info_default() {
        let info = ToolchainInfo::default();
        assert!(info.cabal_version.is_none());
        assert!(info.ghc_version.is_none());
        assert!(!info.ghcup_installed);
        assert!(!info.has_cabal_project);
        assert!(!info.has_stack_yaml);
    }
}
