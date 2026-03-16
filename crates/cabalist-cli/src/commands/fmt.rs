//! `cabalist-cli fmt` — Format the .cabal file.
//!
//! Performs round-trip formatting (parse + render) and optionally sorts
//! dependencies and modules alphabetically.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_opinions::config::find_and_load_config;
use cabalist_parser::ast::{derive_ast, Component};
use cabalist_parser::edit::{self, EditBatch};

use crate::util;

pub fn run(file: &Option<PathBuf>, check: bool) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (original_source, _result) = util::load_and_parse(&cabal_path)?;

    // Load config for formatting preferences.
    let project_root = cabal_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = find_and_load_config(project_root);

    let mut current_source = original_source.clone();

    // Sort dependencies if configured.
    if config.formatting.sort_dependencies {
        current_source = sort_list_field(&current_source, "build-depends");
    }

    // Sort modules if configured.
    if config.formatting.sort_modules {
        current_source = sort_list_field(&current_source, "exposed-modules");
        current_source = sort_list_field(&current_source, "other-modules");
    }

    // Re-parse and render to normalize (the round-trip should be clean).
    let final_result = cabalist_parser::parse(&current_source);
    let formatted = final_result.cst.render();

    if check {
        if formatted != original_source {
            eprintln!("{} needs formatting", cabal_path.display());
            return Ok(ExitCode::from(1));
        }
        println!("{} is correctly formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    if formatted == original_source {
        println!("{} is already formatted", cabal_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    std::fs::write(&cabal_path, &formatted)?;
    println!("Formatted {}", cabal_path.display());
    Ok(ExitCode::SUCCESS)
}

/// A section identifier that survives re-parsing: keyword + optional name.
#[derive(Debug, Clone)]
struct SectionKey {
    keyword: String,
    name: Option<String>,
}

/// Collect stable section identifiers from the AST.
fn collect_section_keys(source: &str) -> Vec<SectionKey> {
    let result = cabalist_parser::parse(source);
    let ast = derive_ast(&result.cst);
    let mut keys = Vec::new();

    for comp in ast.all_components() {
        let key = match comp {
            Component::Library(lib) => SectionKey {
                keyword: "library".to_string(),
                name: lib.fields.name.map(|s| s.to_string()),
            },
            Component::Executable(exe) => SectionKey {
                keyword: "executable".to_string(),
                name: exe.fields.name.map(|s| s.to_string()),
            },
            Component::TestSuite(ts) => SectionKey {
                keyword: "test-suite".to_string(),
                name: ts.fields.name.map(|s| s.to_string()),
            },
            Component::Benchmark(bm) => SectionKey {
                keyword: "benchmark".to_string(),
                name: bm.fields.name.map(|s| s.to_string()),
            },
        };
        keys.push(key);
    }
    for cs in &ast.common_stanzas {
        keys.push(SectionKey {
            keyword: "common".to_string(),
            name: Some(cs.name.to_string()),
        });
    }
    keys
}

/// Re-find a section and field in a freshly parsed source.
fn find_section_field(
    source: &str,
    key: &SectionKey,
    field_name: &str,
) -> Option<(cabalist_parser::ParseResult, cabalist_parser::span::NodeId, cabalist_parser::span::NodeId)> {
    let result = cabalist_parser::parse(source);
    let section_id = edit::find_section(&result.cst, &key.keyword, key.name.as_deref())?;
    let field_id = edit::find_field(&result.cst, section_id, field_name)?;
    Some((result, section_id, field_id))
}

/// Sort items within all instances of a specific list field across all sections.
///
/// For each section containing the target field, reads items, sorts them, and
/// rewrites by removing items one-at-a-time (re-parsing between each removal)
/// then re-adding in sorted order (also one-at-a-time).
fn sort_list_field(source: &str, field_name: &str) -> String {
    let section_keys = collect_section_keys(source);
    let mut current = source.to_string();

    for key in &section_keys {
        // Parse and find the field.
        let Some((result, _section_id, field_id)) =
            find_section_field(&current, key, field_name)
        else {
            continue;
        };

        // Extract current items.
        let items = extract_list_items(&result.cst, field_id);
        if items.len() <= 1 {
            continue;
        }

        // Check if already sorted.
        let mut sorted_items = items.clone();
        sorted_items.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        if items == sorted_items {
            continue;
        }

        // Remove items one at a time, re-parsing between each removal.
        // Remove in reverse order so we don't need to worry about name collisions
        // with items that share prefixes.
        for item in items.iter().rev() {
            let Some((re_result, _re_section, re_field)) =
                find_section_field(&current, key, field_name)
            else {
                break;
            };
            let item_name = item.split_whitespace().next().unwrap_or(item);
            let edits = edit::remove_list_item(&re_result.cst, re_field, item_name);
            if !edits.is_empty() {
                let mut batch = EditBatch::new();
                batch.add_all(edits);
                current = batch.apply(&re_result.cst.source);
            }
        }

        // Re-add items in sorted order, one at a time.
        for item in &sorted_items {
            let Some((re_result, _re_section, re_field)) =
                find_section_field(&current, key, field_name)
            else {
                break;
            };
            let edits = edit::add_list_item(&re_result.cst, re_field, item, false);
            if !edits.is_empty() {
                let mut batch = EditBatch::new();
                batch.add_all(edits);
                current = batch.apply(&re_result.cst.source);
            }
        }
    }

    current
}

/// Extract the individual items from a list field's value.
fn extract_list_items(
    cst: &cabalist_parser::cst::CabalCst,
    field_node: cabalist_parser::span::NodeId,
) -> Vec<String> {
    use cabalist_parser::cst::CstNodeKind;

    let node = &cst.nodes[field_node.0];
    let mut items = Vec::new();

    // Collect value lines from children.
    for &child_id in &node.children {
        let child = &cst.nodes[child_id.0];
        if matches!(child.kind, CstNodeKind::ValueLine) {
            let text = &cst.source[child.span.start..child.span.end];
            let text = text.trim();
            // Strip leading/trailing commas.
            let text = text.trim_start_matches(',').trim_end_matches(',').trim();
            if !text.is_empty() {
                items.push(text.to_string());
            }
        }
    }

    // If no ValueLine children, try the field_value directly.
    if items.is_empty() {
        if let Some(ref val) = node.field_value {
            let text = &cst.source[val.start..val.end];
            let text = text.trim();
            // Split comma-separated single-line values.
            for part in text.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
            }
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_trailing_comma_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
    aeson ^>=2.2,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_leading_comma_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
      text ^>=2.0
    , base ^>=4.17
    , aeson ^>=2.2
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_single_line_deps() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends: text ^>=2.0, base ^>=4.17, aeson ^>=2.2
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(dep_names, vec!["aeson", "base", "text"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_modules_no_comma() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules:
    Zebra
    Alpha
    Middle
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "exposed-modules");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);
        let lib = ast.library.as_ref().unwrap();
        assert_eq!(lib.exposed_modules, vec!["Alpha", "Middle", "Zebra"]);
        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_already_sorted_is_noop() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    aeson ^>=2.2,
    base ^>=4.17,
    text ^>=2.0,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        assert_eq!(sorted, source, "already sorted should be a no-op");
    }

    #[test]
    fn sort_multiple_sections() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
  default-language: GHC2021

executable my-exe
  main-is: Main.hs
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
  default-language: GHC2021
";
        let sorted = sort_list_field(source, "build-depends");
        let result = cabalist_parser::parse(&sorted);
        let ast = derive_ast(&result.cst);

        let lib = ast.library.as_ref().unwrap();
        let lib_deps: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(lib_deps, vec!["base", "text"]);

        let exe = &ast.executables[0];
        let exe_deps: Vec<&str> = exe.fields.build_depends.iter().map(|d| d.package).collect();
        assert_eq!(exe_deps, vec!["base", "text"]);

        assert_eq!(result.cst.render(), sorted);
    }

    #[test]
    fn sort_idempotent() {
        let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    text ^>=2.0,
    base ^>=4.17,
    aeson ^>=2.2,
  default-language: GHC2021
";
        let first = sort_list_field(source, "build-depends");
        let second = sort_list_field(&first, "build-depends");
        assert_eq!(first, second, "sorting must be idempotent");
    }
}
