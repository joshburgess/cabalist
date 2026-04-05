//! Tests for shell completion and man page generation commands.

use std::process::Command;

fn cabalist_cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cabalist-cli"))
}

#[test]
fn completions_bash_produces_output() {
    let output = cabalist_cli()
        .args(["completions", "bash"])
        .output()
        .expect("failed to run cabalist-cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cabalist-cli"), "should contain command name");
    assert!(stdout.contains("complete"), "should contain bash completion directives");
}

#[test]
fn completions_zsh_produces_output() {
    let output = cabalist_cli()
        .args(["completions", "zsh"])
        .output()
        .expect("failed to run cabalist-cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn completions_fish_produces_output() {
    let output = cabalist_cli()
        .args(["completions", "fish"])
        .output()
        .expect("failed to run cabalist-cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn manpages_generates_files() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let output = cabalist_cli()
        .args(["manpages", dir.path().to_str().unwrap()])
        .output()
        .expect("failed to run cabalist-cli");

    assert!(output.status.success());

    // Should generate the main man page plus one per subcommand.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert!(entries.len() > 10, "should generate many man pages, got {}", entries.len());

    // Main page must exist.
    assert!(dir.path().join("cabalist-cli.1").exists());

    // Spot-check a subcommand page.
    assert!(dir.path().join("cabalist-cli-add.1").exists());
    assert!(dir.path().join("cabalist-cli-check.1").exists());
}

#[test]
fn manpages_creates_output_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let nested = dir.path().join("deep").join("nested").join("man");

    let output = cabalist_cli()
        .args(["manpages", nested.to_str().unwrap()])
        .output()
        .expect("failed to run cabalist-cli");

    assert!(output.status.success());
    assert!(nested.join("cabalist-cli.1").exists());
}
