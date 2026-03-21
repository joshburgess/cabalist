//! `cabalist-cli deps` — Show dependency information.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_parser::ast::{derive_ast, Component, VersionRange};
use colored::Colorize;

use crate::util;

pub fn run(file: &Option<PathBuf>, tree: bool, outdated: bool) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let ast = derive_ast(&result.cst);

    if outdated {
        print_outdated_deps(&ast);
    } else if tree {
        print_dependency_tree(&ast);
    } else {
        print_dependency_list(&ast);
    }

    Ok(ExitCode::SUCCESS)
}

fn print_dependency_list(ast: &cabalist_parser::ast::CabalFile<'_>) {
    let components = ast.all_components();

    if components.is_empty() {
        println!("No components found.");
        return;
    }

    for comp in &components {
        let comp_name = component_display_name(comp);
        println!("{}", comp_name.bold());

        let deps = &comp.fields().build_depends;
        if deps.is_empty() {
            println!("  (no dependencies)");
        } else {
            for dep in deps {
                let version_str = match &dep.version_range {
                    Some(vr) => format!("{vr}"),
                    None => "(any)".to_string(),
                };

                let pvp_status = match &dep.version_range {
                    Some(vr) => pvp_check(vr),
                    None => PvpStatus::NoBounds,
                };

                let status_icon = match pvp_status {
                    PvpStatus::Ok => "ok".green().to_string(),
                    PvpStatus::NoUpperBound => "no upper bound".yellow().to_string(),
                    PvpStatus::NoLowerBound => "no lower bound".yellow().to_string(),
                    PvpStatus::NoBounds => "no bounds".yellow().to_string(),
                };

                println!("  {:<30} {:<25} {}", dep.package, version_str, status_icon);
            }
        }
        println!();
    }
}

fn print_dependency_tree(ast: &cabalist_parser::ast::CabalFile<'_>) {
    let components = ast.all_components();

    for (i, comp) in components.iter().enumerate() {
        let comp_name = component_display_name(comp);
        let is_last_comp = i == components.len() - 1;
        let prefix = if is_last_comp {
            "└── "
        } else {
            "├── "
        };
        println!("{}{}", prefix, comp_name.bold());

        let deps = &comp.fields().build_depends;
        let child_prefix = if is_last_comp { "    " } else { "│   " };

        for (j, dep) in deps.iter().enumerate() {
            let is_last_dep = j == deps.len() - 1;
            let dep_prefix = if is_last_dep {
                "└── "
            } else {
                "├── "
            };

            let version_str = match &dep.version_range {
                Some(vr) => format!(" {vr}"),
                None => String::new(),
            };

            println!(
                "{}{}{}{}",
                child_prefix, dep_prefix, dep.package, version_str
            );
        }
    }
}

/// Print outdated dependencies by comparing against the Hackage index.
fn print_outdated_deps(ast: &cabalist_parser::ast::CabalFile<'_>) {
    // Try to load the cached Hackage index.
    let cache_dir = directories::ProjectDirs::from("", "", "cabalist")
        .map(|dirs| dirs.cache_dir().to_path_buf());
    let index = cache_dir.and_then(|dir| {
        let index_path = dir.join("index.json");
        cabalist_hackage::index::HackageIndex::load_from_cache(&index_path).ok()
    });

    let Some(index) = index else {
        eprintln!(
            "{}: Hackage index not found. Run the TUI to download it, \
             or check ~/.cache/cabalist/index.json exists.",
            "warning".yellow().bold()
        );
        eprintln!("Showing dependency list without version comparison.\n");
        print_dependency_list(ast);
        return;
    };

    let components = ast.all_components();
    if components.is_empty() {
        println!("No components found.");
        return;
    }

    let mut any_outdated = false;

    for comp in &components {
        let comp_name = component_display_name(comp);
        let deps = &comp.fields().build_depends;
        if deps.is_empty() {
            continue;
        }

        let mut comp_outdated = Vec::new();
        for dep in deps {
            let Some(latest) = index.latest_version(dep.package) else {
                continue;
            };

            // Check if the current constraint would accept the latest version.
            // Convert hackage Version to parser Version for comparison.
            let parser_version = cabalist_parser::ast::Version {
                components: latest.components.clone(),
            };
            let constrained = match &dep.version_range {
                Some(vr) => !cabalist_parser::ast::version_satisfies(&parser_version, vr),
                None => false, // "any" accepts everything
            };

            if constrained {
                comp_outdated.push((dep.package, &dep.version_range, latest));
            }
        }

        if !comp_outdated.is_empty() {
            any_outdated = true;
            println!("{}", comp_name.bold());
            for (pkg, current_vr, latest) in &comp_outdated {
                let current = match current_vr {
                    Some(vr) => format!("{vr}"),
                    None => "(any)".to_string(),
                };
                println!(
                    "  {:<30} {:<25} → latest: {}",
                    pkg,
                    current,
                    latest.to_string().green()
                );
            }
            println!();
        }
    }

    if !any_outdated {
        println!("{}", "All dependencies are up to date.".green());
    }
}

fn component_display_name(comp: &Component<'_, '_>) -> String {
    match comp {
        Component::Library(lib) => match lib.fields.name {
            Some(name) => format!("library {name}"),
            None => "library".to_string(),
        },
        Component::Executable(exe) => {
            format!("executable {}", exe.fields.name.unwrap_or("(unnamed)"))
        }
        Component::TestSuite(ts) => {
            format!("test-suite {}", ts.fields.name.unwrap_or("(unnamed)"))
        }
        Component::Benchmark(bm) => {
            format!("benchmark {}", bm.fields.name.unwrap_or("(unnamed)"))
        }
    }
}

enum PvpStatus {
    Ok,
    NoUpperBound,
    NoLowerBound,
    NoBounds,
}

fn pvp_check(vr: &VersionRange) -> PvpStatus {
    let has_upper = has_upper_bound(vr);
    let has_lower = has_lower_bound(vr);
    match (has_lower, has_upper) {
        (true, true) => PvpStatus::Ok,
        (true, false) => PvpStatus::NoUpperBound,
        (false, true) => PvpStatus::NoLowerBound,
        (false, false) => PvpStatus::NoBounds,
    }
}

fn has_upper_bound(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => false,
        VersionRange::Eq(_) => true,
        VersionRange::Gt(_) | VersionRange::Gte(_) => false,
        VersionRange::Lt(_) | VersionRange::Lte(_) => true,
        VersionRange::MajorBound(_) => true,
        VersionRange::And(a, b) => has_upper_bound(a) || has_upper_bound(b),
        VersionRange::Or(a, b) => has_upper_bound(a) && has_upper_bound(b),
        VersionRange::NoVersion => true,
    }
}

fn has_lower_bound(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => false,
        VersionRange::Eq(_) => true,
        VersionRange::Gt(_) | VersionRange::Gte(_) => true,
        VersionRange::Lt(_) | VersionRange::Lte(_) => false,
        VersionRange::MajorBound(_) => true,
        VersionRange::And(a, b) => has_lower_bound(a) || has_lower_bound(b),
        VersionRange::Or(a, b) => has_lower_bound(a) && has_lower_bound(b),
        VersionRange::NoVersion => true,
    }
}
