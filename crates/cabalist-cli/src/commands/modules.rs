//! `cabalist-cli modules` — List and scan modules.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use cabalist_parser::ast::{derive_ast, Component};
use colored::Colorize;

use crate::util;

pub fn run(file: &Option<PathBuf>, scan: bool, component: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let ast = derive_ast(&result.cst);

    // Find the requested component.
    let (keyword, name) = util::parse_component_spec(component);
    let comp = find_matching_component(&ast, keyword, name)
        .ok_or_else(|| anyhow::anyhow!("Component '{}' not found", component))?;

    let fields = comp.fields();

    // Print exposed modules (library only).
    if let Component::Library(lib) = &comp {
        if !lib.exposed_modules.is_empty() {
            println!("{}", "Exposed modules:".bold());
            for m in &lib.exposed_modules {
                println!("  {m}");
            }
        }
    }

    // Print other modules.
    if !fields.other_modules.is_empty() {
        println!("{}", "Other modules:".bold());
        for m in &fields.other_modules {
            println!("  {m}");
        }
    }

    // Scan filesystem if requested.
    if scan {
        let project_root = cabal_path.parent().unwrap_or_else(|| Path::new("."));
        scan_for_unlisted_modules(project_root, &comp)?;
    }

    Ok(ExitCode::SUCCESS)
}

fn find_matching_component<'a, 'b>(
    ast: &'b cabalist_parser::ast::CabalFile<'a>,
    keyword: &str,
    name: Option<&str>,
) -> Option<Component<'a, 'b>> {
    match keyword {
        "library" => {
            if let Some(ref lib) = ast.library {
                return Some(Component::Library(lib));
            }
            for lib in &ast.named_libraries {
                if name.is_some() && lib.fields.name == name {
                    return Some(Component::Library(lib));
                }
            }
            None
        }
        "executable" => ast
            .executables
            .iter()
            .find(|e| name.is_none() || e.fields.name == name)
            .map(Component::Executable),
        "test-suite" => ast
            .test_suites
            .iter()
            .find(|t| name.is_none() || t.fields.name == name)
            .map(Component::TestSuite),
        "benchmark" => ast
            .benchmarks
            .iter()
            .find(|b| name.is_none() || b.fields.name == name)
            .map(Component::Benchmark),
        _ => None,
    }
}

/// Scan the filesystem for .hs files not listed in the .cabal file.
fn scan_for_unlisted_modules(project_root: &Path, comp: &Component<'_, '_>) -> Result<()> {
    let fields = comp.fields();

    // Get the source directories.
    let source_dirs: Vec<&str> = if fields.hs_source_dirs.is_empty() {
        vec!["."]
    } else {
        fields.hs_source_dirs.clone()
    };

    // Collect all listed modules.
    let mut listed_modules: Vec<String> = Vec::new();
    if let Component::Library(lib) = comp {
        listed_modules.extend(lib.exposed_modules.iter().map(|s| s.to_string()));
    }
    listed_modules.extend(fields.other_modules.iter().map(|s| s.to_string()));

    // Scan each source directory.
    let mut unlisted = Vec::new();
    for src_dir in &source_dirs {
        let dir = project_root.join(src_dir);
        if dir.is_dir() {
            scan_directory(&dir, &dir, &listed_modules, &mut unlisted)?;
        }
    }

    if unlisted.is_empty() {
        println!(
            "\n{}",
            "All .hs files are listed in the .cabal file.".green()
        );
    } else {
        println!(
            "\n{}",
            format!("Found {} unlisted .hs file(s):", unlisted.len())
                .yellow()
                .bold()
        );
        for (module_name, file_path) in &unlisted {
            println!("  {} ({})", module_name, file_path.display());
        }
    }

    Ok(())
}

/// Recursively scan a directory for .hs files and check if they're listed.
fn scan_directory(
    base: &Path,
    dir: &Path,
    listed: &[String],
    unlisted: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(base, &path, listed, unlisted)?;
        } else if path.extension().is_some_and(|ext| ext == "hs") {
            // Convert file path to module name.
            if let Some(module_name) = path_to_module_name(base, &path) {
                if !listed.iter().any(|m| m == &module_name) {
                    unlisted.push((module_name, path));
                }
            }
        }
    }
    Ok(())
}

/// Convert a file path relative to a source directory into a Haskell module name.
/// e.g., `src/Data/Map.hs` with base `src/` becomes `Data.Map`.
fn path_to_module_name(base: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(base).ok()?;
    let stem = relative.with_extension("");
    let components: Vec<&str> = stem
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    if components.is_empty() {
        return None;
    }
    Some(components.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_path_to_module_name() {
        let base = Path::new("src");
        let file = Path::new("src/Data/Map.hs");
        assert_eq!(
            path_to_module_name(base, file),
            Some("Data.Map".to_string())
        );
    }

    #[test]
    fn test_path_to_module_name_single() {
        let base = Path::new("src");
        let file = Path::new("src/MyLib.hs");
        assert_eq!(path_to_module_name(base, file), Some("MyLib".to_string()));
    }
}
