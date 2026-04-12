//! `cabalist-cli set <field> <value>` — Set a top-level metadata field.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Result};
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

/// Valid cabal-version values.
const VALID_CABAL_VERSIONS: &[&str] = &[
    "1.2", "1.4", "1.6", "1.8", "1.10", "1.12", "1.14", "1.16", "1.18", "1.20", "1.22", "1.24",
    "2.0", "2.2", "2.4", "3.0", "3.4", "3.6", "3.8", "3.12", "3.14",
];

/// Valid build-type values.
const VALID_BUILD_TYPES: &[&str] = &["Simple", "Configure", "Make", "Custom"];

/// Common SPDX license identifiers.
const VALID_LICENSES: &[&str] = &[
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Apache-2.0",
    "ISC",
    "MPL-2.0",
    "GPL-2.0-only",
    "GPL-2.0-or-later",
    "GPL-3.0-only",
    "GPL-3.0-or-later",
    "LGPL-2.1-only",
    "LGPL-3.0-only",
    "AGPL-3.0-only",
    "0BSD",
    "Unlicense",
    "NONE",
];

pub fn run(file: &Option<PathBuf>, field: &str, value: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let (_source, result) = util::load_and_parse(&cabal_path)?;
    let cst = &result.cst;

    let canonical = field.to_ascii_lowercase().replace('_', "-");

    if !SETTABLE_FIELDS.contains(&canonical.as_str()) {
        bail!(
            "Unknown field '{}'. Settable fields: {}",
            field,
            SETTABLE_FIELDS.join(", ")
        );
    }

    // Validate the value for fields with known constraints.
    validate_field_value(&canonical, value)?;

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

/// Validate that a field value is sensible for its field type.
fn validate_field_value(field: &str, value: &str) -> Result<()> {
    match field {
        "version" => {
            // Must be a valid PVP version: digits separated by dots.
            if value.is_empty() {
                bail!("Version cannot be empty");
            }
            for part in value.split('.') {
                if part.parse::<u64>().is_err() {
                    bail!(
                        "Invalid version '{}': each component must be a number (e.g., 0.1.0.0)",
                        value
                    );
                }
            }
        }
        "cabal-version" => {
            if !VALID_CABAL_VERSIONS.contains(&value) {
                eprintln!(
                    "warning: '{}' is not a recognized cabal-version. Common values: {}",
                    value,
                    VALID_CABAL_VERSIONS[VALID_CABAL_VERSIONS.len() - 5..].join(", ")
                );
            }
        }
        "license" => {
            if !VALID_LICENSES.iter().any(|l| l.eq_ignore_ascii_case(value)) {
                eprintln!(
                    "warning: '{}' is not a recognized SPDX license identifier. Common licenses: MIT, BSD-3-Clause, Apache-2.0",
                    value
                );
            }
        }
        "build-type" => {
            if !VALID_BUILD_TYPES
                .iter()
                .any(|b| b.eq_ignore_ascii_case(value))
            {
                bail!(
                    "Invalid build-type '{}'. Must be one of: {}",
                    value,
                    VALID_BUILD_TYPES.join(", ")
                );
            }
        }
        "name" => {
            if value.is_empty() {
                bail!("Package name cannot be empty");
            }
            if value.contains(' ') {
                bail!("Package name cannot contain spaces");
            }
        }
        "homepage" | "bug-reports" => {
            if !value.is_empty() && !value.starts_with("http://") && !value.starts_with("https://")
            {
                eprintln!(
                    "warning: '{}' does not look like a URL (expected http:// or https://)",
                    value
                );
            }
        }
        _ => {} // No validation for free-text fields.
    }
    Ok(())
}
