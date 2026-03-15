//! Parser for `plan.json` produced by `cabal build --dry-run`.

use crate::error::CabalError;
use serde::Deserialize;
use std::path::Path;

/// A parsed solver plan from `plan.json`.
#[derive(Debug, Clone)]
pub struct SolverPlan {
    /// All packages in the install plan.
    pub install_plan: Vec<PlannedPackage>,
    /// The compiler identifier, e.g. `"ghc-9.8.2"`.
    pub compiler: Option<String>,
    /// Total number of packages in the plan.
    pub total_packages: usize,
    /// Number of packages that need to be downloaded.
    pub packages_to_download: usize,
    /// Number of packages that need to be built (not pre-existing).
    pub packages_to_build: usize,
}

/// A single package in the solver plan.
#[derive(Debug, Clone, Deserialize)]
pub struct PlannedPackage {
    /// Package name.
    #[serde(rename = "pkg-name")]
    pub name: String,
    /// Package version string.
    #[serde(rename = "pkg-version")]
    pub version: String,
    /// Plan type: `"configured"`, `"pre-existing"`, `"installed"`.
    #[serde(rename = "type")]
    pub plan_type: String,
    /// Install style: `"global"`, `"local"`, `"inplace"`.
    #[serde(default)]
    pub style: Option<String>,
    /// Component name, if applicable.
    #[serde(rename = "component-name", default)]
    pub component_name: Option<String>,
}

/// Raw plan.json structure for deserialization.
#[derive(Deserialize)]
struct RawPlanJson {
    #[serde(rename = "install-plan")]
    install_plan: Vec<PlannedPackage>,
    #[serde(rename = "compiler-id", default)]
    compiler_id: Option<String>,
}

/// Parse the `plan.json` file at the given path.
pub fn parse_plan_json(path: &Path) -> Result<SolverPlan, CabalError> {
    let content = std::fs::read_to_string(path)?;
    parse_plan_json_content(&content)
}

/// Parse `plan.json` from raw JSON content.
pub fn parse_plan_json_content(content: &str) -> Result<SolverPlan, CabalError> {
    let raw: RawPlanJson = serde_json::from_str(content)?;

    let total_packages = raw.install_plan.len();

    let packages_to_build = raw
        .install_plan
        .iter()
        .filter(|p| p.plan_type != "pre-existing" && p.plan_type != "installed")
        .count();

    // Packages to download are configured packages with global style (from Hackage).
    let packages_to_download = raw
        .install_plan
        .iter()
        .filter(|p| p.plan_type == "configured" && p.style.as_deref() == Some("global"))
        .count();

    Ok(SolverPlan {
        install_plan: raw.install_plan,
        compiler: raw.compiler_id,
        total_packages,
        packages_to_download,
        packages_to_build,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_plan() {
        let json = r#"{
            "install-plan": [
                {
                    "type": "pre-existing",
                    "pkg-name": "base",
                    "pkg-version": "4.19.1.0"
                },
                {
                    "type": "configured",
                    "pkg-name": "aeson",
                    "pkg-version": "2.2.1.0",
                    "style": "global"
                },
                {
                    "type": "configured",
                    "pkg-name": "my-project",
                    "pkg-version": "0.1.0.0",
                    "style": "local",
                    "component-name": "lib"
                }
            ],
            "compiler-id": "ghc-9.8.2"
        }"#;

        let plan = parse_plan_json_content(json).unwrap();
        assert_eq!(plan.total_packages, 3);
        assert_eq!(plan.packages_to_build, 2); // aeson + my-project
        assert_eq!(plan.packages_to_download, 1); // aeson (global)
        assert_eq!(plan.compiler.as_deref(), Some("ghc-9.8.2"));

        assert_eq!(plan.install_plan[0].name, "base");
        assert_eq!(plan.install_plan[0].plan_type, "pre-existing");
        assert_eq!(plan.install_plan[1].name, "aeson");
        assert_eq!(plan.install_plan[1].style.as_deref(), Some("global"));
        assert_eq!(plan.install_plan[2].component_name.as_deref(), Some("lib"));
    }

    #[test]
    fn parse_empty_install_plan() {
        let json = r#"{
            "install-plan": [],
            "compiler-id": "ghc-9.8.2"
        }"#;

        let plan = parse_plan_json_content(json).unwrap();
        assert_eq!(plan.total_packages, 0);
        assert_eq!(plan.packages_to_build, 0);
        assert_eq!(plan.packages_to_download, 0);
        assert!(plan.install_plan.is_empty());
    }

    #[test]
    fn parse_missing_optional_fields() {
        let json = r#"{
            "install-plan": [
                {
                    "type": "configured",
                    "pkg-name": "text",
                    "pkg-version": "2.0.2"
                }
            ]
        }"#;

        let plan = parse_plan_json_content(json).unwrap();
        assert_eq!(plan.total_packages, 1);
        assert!(plan.compiler.is_none());
        assert!(plan.install_plan[0].style.is_none());
        assert!(plan.install_plan[0].component_name.is_none());
    }

    #[test]
    fn count_packages_correctly() {
        let json = r#"{
            "install-plan": [
                { "type": "pre-existing", "pkg-name": "base", "pkg-version": "4.19.1.0" },
                { "type": "pre-existing", "pkg-name": "ghc-prim", "pkg-version": "0.11.0" },
                { "type": "installed", "pkg-name": "text", "pkg-version": "2.0.2" },
                { "type": "configured", "pkg-name": "aeson", "pkg-version": "2.2.1.0", "style": "global" },
                { "type": "configured", "pkg-name": "vector", "pkg-version": "0.13.1.0", "style": "global" },
                { "type": "configured", "pkg-name": "my-app", "pkg-version": "0.1.0.0", "style": "local" }
            ],
            "compiler-id": "ghc-9.8.2"
        }"#;

        let plan = parse_plan_json_content(json).unwrap();
        assert_eq!(plan.total_packages, 6);
        assert_eq!(plan.packages_to_build, 3); // aeson + vector + my-app (not pre-existing/installed)
        assert_eq!(plan.packages_to_download, 2); // aeson + vector (global configured)
    }

    #[test]
    fn invalid_json_returns_error() {
        let result = parse_plan_json_content("not json");
        assert!(result.is_err());
    }
}
