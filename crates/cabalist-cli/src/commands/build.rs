//! `cabalist-cli build/test/clean` — Run cabal commands.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use colored::Colorize;

use crate::util;

pub fn run_build(file: &Option<PathBuf>) -> Result<ExitCode> {
    run_cabal_command(file, "build")
}

pub fn run_test(file: &Option<PathBuf>) -> Result<ExitCode> {
    run_cabal_command(file, "test")
}

pub fn run_clean(file: &Option<PathBuf>) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let working_dir = cabal_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match cabalist_cabal::cabal_clean(&working_dir).await {
            Ok(()) => {
                println!("{}", "Clean completed.".green());
                Ok(ExitCode::SUCCESS)
            }
            Err(e) => {
                eprintln!("{}: {e}", "error".red().bold());
                Ok(ExitCode::from(1))
            }
        }
    })
}

fn run_cabal_command(file: &Option<PathBuf>, command: &str) -> Result<ExitCode> {
    let cabal_path = util::resolve_cabal_file(file)?;
    let working_dir = cabal_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let (line_tx, mut line_rx) =
            tokio::sync::mpsc::unbounded_channel::<cabalist_cabal::OutputLine>();

        // Spawn a task to print lines as they arrive.
        let printer = tokio::spawn(async move {
            while let Some(line) = line_rx.recv().await {
                let text = match line {
                    cabalist_cabal::OutputLine::Stdout(s) => s,
                    cabalist_cabal::OutputLine::Stderr(s) => s,
                };
                println!("{text}");
            }
        });

        let opts = cabalist_cabal::BuildOptions::default();

        let (success, duration) = if command == "test" {
            match cabalist_cabal::cabal_test(&working_dir, &opts, Some(line_tx)).await {
                Ok(r) => (r.success, r.output.duration),
                Err(e) => {
                    let _ = printer.await;
                    eprintln!("{}: {e}", "error".red().bold());
                    return Ok(ExitCode::from(2));
                }
            }
        } else {
            match cabalist_cabal::cabal_build(&working_dir, &opts, Some(line_tx)).await {
                Ok(r) => (r.success, r.output.duration),
                Err(e) => {
                    let _ = printer.await;
                    eprintln!("{}: {e}", "error".red().bold());
                    return Ok(ExitCode::from(2));
                }
            }
        };

        // Wait for printer to finish.
        let _ = printer.await;

        let status = if success {
            format!("{} succeeded", command).green().to_string()
        } else {
            format!("{} FAILED", command).red().to_string()
        };
        println!("\n{} in {:.1}s", status, duration.as_secs_f64());

        if success {
            Ok(ExitCode::SUCCESS)
        } else {
            Ok(ExitCode::from(1))
        }
    })
}
