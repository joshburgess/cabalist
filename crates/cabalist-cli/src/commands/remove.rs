//! `cabalist-cli remove <package>` — Remove a dependency.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_parser::edit::{self, EditBatch};

use crate::util;

pub fn run(file: &Option<PathBuf>, package: &str, component: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;

    let cst = &result.cst;

    // Find the target section.
    let (keyword, name) = util::parse_component_spec(component);
    let section_id = edit::find_section(cst, keyword, name).ok_or_else(|| {
        anyhow::anyhow!(
            "Component '{}' not found in {}",
            component,
            cabal_path.display()
        )
    })?;

    // Find the build-depends field.
    let field_id = edit::find_field(cst, section_id, "build-depends").ok_or_else(|| {
        anyhow::anyhow!(
            "No 'build-depends' field found in component '{}'",
            component
        )
    })?;

    // Generate removal edits.
    let edits = edit::remove_list_item(cst, field_id, package);
    if edits.is_empty() {
        eprintln!(
            "Package '{}' not found in {}'s build-depends",
            package, component
        );
        return Ok(ExitCode::from(1));
    }

    // Apply edits.
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let new_source = batch.apply(&cst.source);

    // Write back.
    std::fs::write(&cabal_path, &new_source)?;

    println!(
        "Removed '{}' from {} in {}",
        package,
        component,
        cabal_path.display()
    );
    Ok(ExitCode::SUCCESS)
}
