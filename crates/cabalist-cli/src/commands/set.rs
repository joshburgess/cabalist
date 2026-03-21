//! `cabalist-cli set <field> <value>` — Set a top-level metadata field.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use cabalist_parser::edit::{self, EditBatch};

use crate::util;

/// Fields that can be set via the `set` command.
const SETTABLE_FIELDS: &[&str] = &[
    "name",
    "version",
    "cabal-version",
    "synopsis",
    "description",
    "license",
    "license-file",
    "author",
    "maintainer",
    "homepage",
    "bug-reports",
    "category",
    "build-type",
    "tested-with",
];

pub fn run(file: &Option<PathBuf>, field: &str, value: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let cst = &result.cst;

    let canonical = field.to_ascii_lowercase().replace('_', "-");

    if !SETTABLE_FIELDS.contains(&canonical.as_str()) {
        anyhow::bail!(
            "Unknown field '{}'. Settable fields: {}",
            field,
            SETTABLE_FIELDS.join(", ")
        );
    }

    let edits = if let Some(field_id) = edit::find_field(cst, cst.root, &canonical) {
        vec![edit::set_field_value(cst, field_id, value)]
    } else {
        vec![edit::add_field_to_root(cst, &canonical, value)]
    };

    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let new_source = batch.apply(&cst.source);
    std::fs::write(&cabal_path, &new_source)?;

    println!(
        "Set '{}' to '{}' in {}",
        canonical,
        value,
        cabal_path.display()
    );
    Ok(ExitCode::SUCCESS)
}
