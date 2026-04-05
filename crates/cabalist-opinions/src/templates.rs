//! Project templates for `cabalist init`.
//!
//! Templates are `.cabal.tmpl` files embedded via `include_str!` with simple
//! `{{variable}}` placeholder substitution.

/// Available project template types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    /// Pure library, no executables.
    Library,
    /// Executable only, no library.
    Application,
    /// Library + executable (most common pattern).
    LibAndExe,
    /// Library + executable + test-suite + benchmark.
    Full,
}

impl TemplateKind {
    /// Return all available template kinds.
    pub fn all() -> &'static [TemplateKind] {
        &[
            TemplateKind::Library,
            TemplateKind::Application,
            TemplateKind::LibAndExe,
            TemplateKind::Full,
        ]
    }

    /// Human-readable label for this template kind.
    pub fn label(&self) -> &'static str {
        match self {
            TemplateKind::Library => "Library",
            TemplateKind::Application => "Application",
            TemplateKind::LibAndExe => "Library + Application",
            TemplateKind::Full => "Full (Library + Exe + Test + Bench)",
        }
    }

    /// Short identifier for CLI usage.
    pub fn id(&self) -> &'static str {
        match self {
            TemplateKind::Library => "lib",
            TemplateKind::Application => "exe",
            TemplateKind::LibAndExe => "lib-exe",
            TemplateKind::Full => "full",
        }
    }

    /// Parse a template kind from its short ID.
    pub fn from_id(id: &str) -> Option<TemplateKind> {
        match id {
            "lib" | "library" => Some(TemplateKind::Library),
            "exe" | "application" => Some(TemplateKind::Application),
            "lib-exe" | "lib-and-exe" => Some(TemplateKind::LibAndExe),
            "full" => Some(TemplateKind::Full),
            _ => None,
        }
    }
}

impl std::fmt::Display for TemplateKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Variables to substitute in a template.
#[derive(Debug, Clone)]
pub struct TemplateVars {
    /// Project / package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// One-line synopsis.
    pub synopsis: String,
    /// Longer description.
    pub description: String,
    /// License identifier (e.g. `"MIT"`).
    pub license: String,
    /// Author name.
    pub author: String,
    /// Maintainer email or name.
    pub maintainer: String,
    /// Hackage category.
    pub category: String,
    /// Repository URL for `source-repository` section.
    pub repo_url: String,
    /// Default language (e.g. `"GHC2021"`).
    pub language: String,
    /// Exposed modules for the library component.
    pub exposed_modules: String,
    /// Base library version bound (e.g. `"4.20"`).
    pub base_version: String,
}

impl Default for TemplateVars {
    fn default() -> Self {
        Self {
            name: "my-project".to_string(),
            version: "0.1.0.0".to_string(),
            synopsis: "A short synopsis".to_string(),
            description: "A longer description".to_string(),
            license: crate::defaults::DEFAULT_LICENSE.to_string(),
            author: "Author Name".to_string(),
            maintainer: "author@example.com".to_string(),
            category: "Development".to_string(),
            repo_url: "https://github.com/user/my-project".to_string(),
            language: crate::defaults::DEFAULT_LANGUAGE.to_string(),
            exposed_modules: "MyLib".to_string(),
            base_version: "4.20".to_string(),
        }
    }
}

// Embedded template sources.
const LIBRARY_TEMPLATE: &str = include_str!("../../../data/templates/library.cabal.tmpl");
const APPLICATION_TEMPLATE: &str = include_str!("../../../data/templates/application.cabal.tmpl");
const LIB_AND_EXE_TEMPLATE: &str = include_str!("../../../data/templates/lib-and-exe.cabal.tmpl");
const FULL_TEMPLATE: &str = include_str!("../../../data/templates/full.cabal.tmpl");

/// Get the raw template string for a given template kind.
pub fn raw_template(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Library => LIBRARY_TEMPLATE,
        TemplateKind::Application => APPLICATION_TEMPLATE,
        TemplateKind::LibAndExe => LIB_AND_EXE_TEMPLATE,
        TemplateKind::Full => FULL_TEMPLATE,
    }
}

/// Render a project template with the given variables.
///
/// Performs simple `{{variable}}` placeholder substitution.
pub fn render_template(kind: TemplateKind, vars: &TemplateVars) -> String {
    let template = raw_template(kind);
    substitute(template, vars)
}

/// Perform placeholder substitution on a template string.
fn substitute(template: &str, vars: &TemplateVars) -> String {
    template
        .replace("{{name}}", &vars.name)
        .replace("{{version}}", &vars.version)
        .replace("{{synopsis}}", &vars.synopsis)
        .replace("{{description}}", &vars.description)
        .replace("{{license}}", &vars.license)
        .replace("{{author}}", &vars.author)
        .replace("{{maintainer}}", &vars.maintainer)
        .replace("{{category}}", &vars.category)
        .replace("{{repo-url}}", &vars.repo_url)
        .replace("{{language}}", &vars.language)
        .replace("{{exposed-modules}}", &vars.exposed_modules)
        .replace("{{base-version}}", &vars.base_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_library_template() {
        let vars = TemplateVars {
            name: "test-lib".to_string(),
            ..Default::default()
        };
        let output = render_template(TemplateKind::Library, &vars);
        assert!(output.contains("name:          test-lib"));
        assert!(output.contains("cabal-version: 3.0"));
        assert!(output.contains("library"));
        assert!(output.contains("exposed-modules:"));
    }

    #[test]
    fn render_application_template() {
        let vars = TemplateVars {
            name: "my-app".to_string(),
            ..Default::default()
        };
        let output = render_template(TemplateKind::Application, &vars);
        assert!(output.contains("name:          my-app"));
        assert!(output.contains("executable my-app"));
        assert!(output.contains("main-is:          Main.hs"));
    }

    #[test]
    fn render_lib_and_exe_template() {
        let vars = TemplateVars {
            name: "my-project".to_string(),
            ..Default::default()
        };
        let output = render_template(TemplateKind::LibAndExe, &vars);
        assert!(output.contains("library"));
        assert!(output.contains("executable my-project"));
    }

    #[test]
    fn render_full_template() {
        let vars = TemplateVars {
            name: "my-project".to_string(),
            ..Default::default()
        };
        let output = render_template(TemplateKind::Full, &vars);
        assert!(output.contains("library"));
        assert!(output.contains("executable my-project"));
        assert!(output.contains("test-suite my-project-tests"));
        assert!(output.contains("benchmark my-project-bench"));
    }

    #[test]
    fn all_templates_non_empty() {
        for kind in TemplateKind::all() {
            let raw = raw_template(*kind);
            assert!(!raw.is_empty(), "Template {:?} is empty", kind);
        }
    }

    #[test]
    fn variable_substitution() {
        let vars = TemplateVars {
            name: "custom-name".to_string(),
            license: "BSD-3-Clause".to_string(),
            author: "Jane Doe".to_string(),
            language: "Haskell2010".to_string(),
            ..Default::default()
        };
        let output = render_template(TemplateKind::Library, &vars);
        assert!(output.contains("custom-name"));
        assert!(output.contains("BSD-3-Clause"));
        assert!(output.contains("Jane Doe"));
        assert!(output.contains("Haskell2010"));
        // No unsubstituted placeholders should remain for the vars we set.
        assert!(!output.contains("{{name}}"));
        assert!(!output.contains("{{license}}"));
        assert!(!output.contains("{{author}}"));
        assert!(!output.contains("{{language}}"));
    }

    #[test]
    fn template_kind_from_id() {
        assert_eq!(TemplateKind::from_id("lib"), Some(TemplateKind::Library));
        assert_eq!(
            TemplateKind::from_id("library"),
            Some(TemplateKind::Library)
        );
        assert_eq!(
            TemplateKind::from_id("exe"),
            Some(TemplateKind::Application)
        );
        assert_eq!(
            TemplateKind::from_id("lib-exe"),
            Some(TemplateKind::LibAndExe)
        );
        assert_eq!(TemplateKind::from_id("full"), Some(TemplateKind::Full));
        assert_eq!(TemplateKind::from_id("unknown"), None);
    }
}
