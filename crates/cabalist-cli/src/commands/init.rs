//! `cabalist-cli init` — Create a new Haskell project.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};
use cabalist_opinions::templates::{render_template, TemplateKind, TemplateVars};
use cabalist_opinions::DEFAULT_LAYOUT;

use crate::ProjectType;

pub fn run(
    name: Option<String>,
    project_type: ProjectType,
    license: String,
    author: Option<String>,
    minimal: bool,
) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    // Determine project name.
    let name = name.unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-project")
            .to_string()
    });

    // Detect author from git config.
    let (author_name, author_email) = detect_git_author();
    let author = author.unwrap_or(author_name);
    let maintainer = author_email;

    // Map ProjectType to TemplateKind.
    let template_kind = match project_type {
        ProjectType::Library => TemplateKind::Library,
        ProjectType::Application => TemplateKind::Application,
        ProjectType::LibAndExe => TemplateKind::LibAndExe,
        ProjectType::Full => TemplateKind::Full,
    };

    // Detect GHC version to choose the appropriate default language and base version.
    let ghc_version = cabalist_ghc::versions::detect_ghc_version();
    let language = ghc_version
        .as_deref()
        .map(cabalist_opinions::defaults::language_for_ghc_version)
        .unwrap_or(cabalist_opinions::DEFAULT_LANGUAGE)
        .to_string();

    let base_version = ghc_version
        .as_deref()
        .and_then(detect_base_major_version)
        .unwrap_or_else(|| "4.20".to_string());

    // Build template variables.
    let vars = TemplateVars {
        name: name.clone(),
        license,
        author,
        maintainer,
        language,
        base_version,
        ..Default::default()
    };

    // Render the template.
    let cabal_content = render_template(template_kind, &vars);

    // Write the .cabal file.
    let cabal_path = cwd.join(format!("{name}.cabal"));
    if cabal_path.exists() {
        anyhow::bail!(
            "{} already exists; refusing to overwrite",
            cabal_path.display()
        );
    }
    std::fs::write(&cabal_path, &cabal_content)
        .with_context(|| format!("Failed to write {}", cabal_path.display()))?;
    println!("Created {}", cabal_path.display());

    // Create directory structure (unless --minimal).
    if !minimal {
        create_project_dirs(&cwd, &name, template_kind)?;
    }

    println!("Initialized {} project '{}'", template_kind.label(), name);
    Ok(ExitCode::SUCCESS)
}

/// Create the standard project directories and stub files.
fn create_project_dirs(root: &Path, name: &str, kind: TemplateKind) -> Result<()> {
    let needs_lib = matches!(
        kind,
        TemplateKind::Library | TemplateKind::LibAndExe | TemplateKind::Full
    );
    let needs_exe = matches!(
        kind,
        TemplateKind::Application | TemplateKind::LibAndExe | TemplateKind::Full
    );
    let needs_test = matches!(kind, TemplateKind::Full);
    let needs_bench = matches!(kind, TemplateKind::Full);

    if needs_lib {
        let src_dir = root.join(DEFAULT_LAYOUT.library_src);
        std::fs::create_dir_all(&src_dir)?;
        let lib_file = src_dir.join("MyLib.hs");
        if !lib_file.exists() {
            std::fs::write(
                &lib_file,
                "module MyLib (someFunc) where\n\nsomeFunc :: IO ()\nsomeFunc = putStrLn \"someFunc\"\n",
            )?;
            println!("  Created {}", lib_file.display());
        }
    }

    if needs_exe {
        let app_dir = root.join(DEFAULT_LAYOUT.executable_src);
        std::fs::create_dir_all(&app_dir)?;
        let main_file = app_dir.join("Main.hs");
        if !main_file.exists() {
            let body = if needs_lib {
                "module Main where\n\nimport MyLib (someFunc)\n\nmain :: IO ()\nmain = someFunc\n"
                    .to_string()
            } else {
                "module Main where\n\nmain :: IO ()\nmain = putStrLn \"Hello, Haskell!\"\n"
                    .to_string()
            };
            std::fs::write(&main_file, body)?;
            println!("  Created {}", main_file.display());
        }
    }

    if needs_test {
        let test_dir = root.join(DEFAULT_LAYOUT.test_src);
        std::fs::create_dir_all(&test_dir)?;
        let test_file = test_dir.join("Main.hs");
        if !test_file.exists() {
            std::fs::write(
                &test_file,
                "module Main where\n\nmain :: IO ()\nmain = putStrLn \"Tests not yet implemented\"\n",
            )?;
            println!("  Created {}", test_file.display());
        }
    }

    if needs_bench {
        let bench_dir = root.join(DEFAULT_LAYOUT.benchmark_src);
        std::fs::create_dir_all(&bench_dir)?;
        let bench_file = bench_dir.join("Main.hs");
        if !bench_file.exists() {
            std::fs::write(
                &bench_file,
                "module Main where\n\nmain :: IO ()\nmain = putStrLn \"Benchmarks not yet implemented\"\n",
            )?;
            println!("  Created {}", bench_file.display());
        }
    }

    // Create cabal.project if it doesn't exist.
    let cabal_project = root.join("cabal.project");
    if !cabal_project.exists() {
        std::fs::write(&cabal_project, "packages: .\n")?;
        println!("  Created {}", cabal_project.display());
    }

    // Create CHANGELOG.md if it doesn't exist.
    let changelog = root.join("CHANGELOG.md");
    if !changelog.exists() {
        std::fs::write(
            &changelog,
            format!("# Changelog for {name}\n\n## 0.1.0.0\n\n* Initial release\n"),
        )?;
        println!("  Created {}", changelog.display());
    }

    let _ = name;
    Ok(())
}

/// Detect the major `base` library version (e.g., "4.20") for the installed GHC.
///
/// Tries an exact lookup first, then searches for the closest known GHC version.
fn detect_base_major_version(ghc_version: &str) -> Option<String> {
    // Try exact match first.
    if let Some(base) = cabalist_ghc::versions::base_version_for_ghc(ghc_version) {
        let parts: Vec<&str> = base.split('.').collect();
        if parts.len() >= 2 {
            return Some(format!("{}.{}", parts[0], parts[1]));
        }
    }

    // Find the closest known GHC version that's <= the installed one.
    let map = cabalist_ghc::versions::ghc_base_map();
    let mut best: Option<&cabalist_ghc::GhcBaseMapping> = None;
    for entry in map {
        if cabalist_ghc::versions::version_gte(ghc_version, entry.ghc) {
            match best {
                Some(prev) if cabalist_ghc::versions::version_gte(entry.ghc, prev.ghc) => {
                    best = Some(entry);
                }
                None => best = Some(entry),
                _ => {}
            }
        }
    }

    best.map(|entry| {
        let parts: Vec<&str> = entry.base.split('.').collect();
        if parts.len() >= 2 {
            format!("{}.{}", parts[0], parts[1])
        } else {
            entry.base.to_string()
        }
    })
}

/// Detect author name and email from git config.
fn detect_git_author() -> (String, String) {
    let name = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "Author Name".to_string());

    let email = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "author@example.com".to_string());

    (name, email)
}
