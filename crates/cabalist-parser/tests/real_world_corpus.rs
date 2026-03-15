//! Round-trip tests against real-world .cabal files from popular Haskell packages.
//!
//! These files were downloaded from GitHub repositories of widely-used packages.
//! The test verifies that `parse → render` produces byte-identical output for each file.

use cabalist_parser::parse;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/cabalist-parser -> crates
    path.pop(); // crates -> workspace root
    path.push("tests");
    path.push("fixtures");
    path.push("real-world");
    path
}

fn assert_round_trip_file(filename: &str) {
    let path = fixtures_dir().join(filename);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

    let result = parse(&source);
    let rendered = result.cst.render();

    if rendered != source {
        // Find first difference for debugging
        let source_bytes = source.as_bytes();
        let rendered_bytes = rendered.as_bytes();
        let first_diff = source_bytes
            .iter()
            .zip(rendered_bytes.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(source_bytes.len().min(rendered_bytes.len()));

        let line = source[..first_diff].matches('\n').count() + 1;
        let line_start = source[..first_diff].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let col = first_diff - line_start;

        let context_start = first_diff.saturating_sub(40);
        let context_end = (first_diff + 40).min(source.len());
        let rendered_context_end = (first_diff + 40).min(rendered.len());

        panic!(
            "Round-trip failed for {} at line {}:{} (byte offset {})\n\
             --- source context ---\n{:?}\n\
             --- rendered context ---\n{:?}\n\
             source len: {}, rendered len: {}",
            filename,
            line,
            col,
            first_diff,
            &source[context_start..context_end],
            &rendered[context_start..rendered_context_end],
            source.len(),
            rendered.len(),
        );
    }
}

fn assert_parses_without_errors(filename: &str) {
    let path = fixtures_dir().join(filename);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

    let result = parse(&source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Error)
        .collect();

    assert!(
        errors.is_empty(),
        "{} had parse errors: {:?}",
        filename,
        errors
    );
}

// ---- Round-trip tests for each real-world .cabal file ----

#[test]
fn round_trip_aeson() {
    assert_round_trip_file("aeson.cabal");
}

#[test]
fn round_trip_lens() {
    assert_round_trip_file("lens.cabal");
}

#[test]
fn round_trip_pandoc() {
    assert_round_trip_file("pandoc.cabal");
}

#[test]
fn round_trip_conduit() {
    assert_round_trip_file("conduit.cabal");
}

#[test]
fn round_trip_servant() {
    assert_round_trip_file("servant.cabal");
}

#[test]
fn round_trip_warp() {
    assert_round_trip_file("warp.cabal");
}

#[test]
fn round_trip_megaparsec() {
    assert_round_trip_file("megaparsec.cabal");
}

#[test]
fn round_trip_optparse_applicative() {
    assert_round_trip_file("optparse-applicative.cabal");
}

#[test]
fn round_trip_wreq() {
    assert_round_trip_file("wreq.cabal");
}

#[test]
fn round_trip_attoparsec() {
    assert_round_trip_file("attoparsec.cabal");
}

#[test]
fn round_trip_stm() {
    assert_round_trip_file("stm.cabal");
}

#[test]
fn round_trip_async() {
    assert_round_trip_file("async.cabal");
}

#[test]
fn round_trip_quickcheck() {
    assert_round_trip_file("QuickCheck.cabal");
}

#[test]
fn round_trip_hedgehog() {
    assert_round_trip_file("hedgehog.cabal");
}

#[test]
fn round_trip_yesod() {
    assert_round_trip_file("yesod.cabal");
}

// ---- Round-trip tests for core libraries ----

#[test]
fn round_trip_vector() {
    assert_round_trip_file("vector.cabal");
}

#[test]
fn round_trip_text() {
    assert_round_trip_file("text.cabal");
}

#[test]
fn round_trip_bytestring() {
    assert_round_trip_file("bytestring.cabal");
}

#[test]
fn round_trip_mtl() {
    assert_round_trip_file("mtl.cabal");
}

#[test]
fn round_trip_tasty() {
    assert_round_trip_file("tasty.cabal");
}

#[test]
fn round_trip_hashable() {
    assert_round_trip_file("hashable.cabal");
}

#[test]
fn round_trip_filepath() {
    assert_round_trip_file("filepath.cabal");
}

#[test]
fn round_trip_containers() {
    assert_round_trip_file("containers.cabal");
}

// ---- Parse-without-errors tests ----

#[test]
fn no_errors_aeson() {
    assert_parses_without_errors("aeson.cabal");
}

#[test]
fn no_errors_lens() {
    assert_parses_without_errors("lens.cabal");
}

#[test]
fn no_errors_pandoc() {
    assert_parses_without_errors("pandoc.cabal");
}

#[test]
fn no_errors_conduit() {
    assert_parses_without_errors("conduit.cabal");
}

#[test]
fn no_errors_servant() {
    assert_parses_without_errors("servant.cabal");
}

#[test]
fn no_errors_warp() {
    assert_parses_without_errors("warp.cabal");
}

#[test]
fn no_errors_quickcheck() {
    assert_parses_without_errors("QuickCheck.cabal");
}

#[test]
fn no_errors_vector() {
    assert_parses_without_errors("vector.cabal");
}

#[test]
fn no_errors_text() {
    assert_parses_without_errors("text.cabal");
}

#[test]
fn no_errors_bytestring() {
    assert_parses_without_errors("bytestring.cabal");
}

#[test]
fn no_errors_hashable() {
    assert_parses_without_errors("hashable.cabal");
}

#[test]
fn no_errors_filepath() {
    assert_parses_without_errors("filepath.cabal");
}

// ---- Aggregate test: all fixtures ----

#[test]
fn all_fixtures_round_trip() {
    let dir = fixtures_dir();
    if !dir.exists() {
        panic!("Fixtures directory not found: {}", dir.display());
    }

    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "cabal")
                .unwrap_or(false)
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    files.sort();

    assert!(
        !files.is_empty(),
        "No .cabal fixture files found in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    for filename in &files {
        let path = dir.join(filename);
        let source = fs::read_to_string(&path).unwrap();
        let result = parse(&source);
        let rendered = result.cst.render();
        if rendered != source {
            let first_diff = source
                .as_bytes()
                .iter()
                .zip(rendered.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or(source.len().min(rendered.len()));
            let line = source[..first_diff].matches('\n').count() + 1;
            failures.push(format!(
                "{}: first diff at line {} (byte {}), source={} bytes, rendered={} bytes",
                filename,
                line,
                first_diff,
                source.len(),
                rendered.len()
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{}/{} real-world .cabal files failed round-trip:\n{}",
            failures.len(),
            files.len(),
            failures.join("\n")
        );
    }

    eprintln!(
        "All {}/{} real-world .cabal files pass round-trip",
        files.len(),
        files.len()
    );
}
