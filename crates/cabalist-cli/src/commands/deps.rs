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

#[derive(Debug)]
enum DepStatus {
    /// Constraint excludes the latest version.
    Outdated,
    /// Latest version satisfies constraint, but a newer version exists.
    Current,
    /// Package not found in the Hackage index.
    Unknown,
}

/// Print outdated dependencies by comparing against the Hackage index.
fn print_outdated_deps(ast: &cabalist_parser::ast::CabalFile<'_>) {
    let cache_dir = directories::ProjectDirs::from("", "", "cabalist")
        .map(|dirs| dirs.cache_dir().to_path_buf());
    let index = cache_dir.and_then(|dir| {
        let index_path = dir.join("index.json");
        cabalist_hackage::index::HackageIndex::load_from_cache(&index_path).ok()
    });

    let Some(index) = index else {
        eprintln!(
            "{}: Hackage index not found. Run {} to download it.",
            "warning".yellow().bold(),
            "cabalist-cli update-index".bold()
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

    // Collect all unique dependencies across components.
    let mut seen = std::collections::HashSet::new();
    let mut rows: Vec<(&str, Option<&VersionRange>, String, DepStatus)> = Vec::new();

    for comp in &components {
        for dep in &comp.fields().build_depends {
            if !seen.insert(dep.package) {
                continue;
            }

            let Some(latest) = index.latest_version(dep.package) else {
                rows.push((dep.package, dep.version_range.as_ref(), "?".to_string(), DepStatus::Unknown));
                continue;
            };

            let latest_str = latest.to_string();
            let parser_version = cabalist_parser::ast::Version {
                components: latest.components.clone(),
            };

            let status = match &dep.version_range {
                Some(vr) => {
                    if cabalist_parser::ast::version_satisfies(&parser_version, vr) {
                        DepStatus::Current
                    } else {
                        DepStatus::Outdated
                    }
                }
                None => DepStatus::Current,
            };

            rows.push((dep.package, dep.version_range.as_ref(), latest_str, status));
        }
    }

    // Compute column widths.
    let pkg_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(7).max(7);
    let constraint_width = rows
        .iter()
        .map(|r| match r.1 {
            Some(vr) => format!("{vr}").len(),
            None => 5,
        })
        .max()
        .unwrap_or(10)
        .max(10);
    let latest_width = rows.iter().map(|r| r.2.len()).max().unwrap_or(6).max(6);

    // Print header.
    println!(
        "  {:<pkg_width$}  {:<constraint_width$}  {:<latest_width$}  {}",
        "Package".bold(),
        "Constraint".bold(),
        "Latest".bold(),
        "Status".bold(),
    );
    println!(
        "  {:<pkg_width$}  {:<constraint_width$}  {:<latest_width$}  ──────",
        "─".repeat(pkg_width),
        "─".repeat(constraint_width),
        "─".repeat(latest_width),
    );

    let mut outdated_count = 0;
    let mut unknown_count = 0;

    for (pkg, constraint, latest, status) in &rows {
        let constraint_str = match constraint {
            Some(vr) => format!("{vr}"),
            None => "(any)".to_string(),
        };

        let (status_str, latest_colored) = match status {
            DepStatus::Outdated => {
                outdated_count += 1;
                ("outdated".red().bold().to_string(), latest.red().to_string())
            }
            DepStatus::Current => ("ok".green().to_string(), latest.to_string()),
            DepStatus::Unknown => {
                unknown_count += 1;
                ("unknown".dimmed().to_string(), latest.dimmed().to_string())
            }
        };

        println!(
            "  {:<pkg_width$}  {:<constraint_width$}  {:<latest_width$}  {}",
            pkg, constraint_str, latest_colored, status_str,
        );
    }

    // Summary line.
    println!();
    let total = rows.len();
    let current = total - outdated_count - unknown_count;
    if outdated_count > 0 {
        println!(
            "  {} outdated, {} up to date, {} total",
            format!("{outdated_count}").red().bold(),
            format!("{current}").green(),
            total
        );
    } else {
        println!("  {}", "All dependencies are up to date.".green());
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
