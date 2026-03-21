//! `cabalist-cli update-index` — Download/refresh the Hackage package index.

use std::process::ExitCode;

use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<ExitCode> {
    let cache_dir = directories::ProjectDirs::from("", "", "cabalist")
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?;

    println!(
        "Downloading Hackage index to {}...",
        cache_dir.display()
    );

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match cabalist_hackage::client::update_index(&cache_dir).await {
            Ok(index) => {
                println!(
                    "{} Index updated: {} packages",
                    "done.".green().bold(),
                    index.len()
                );
                Ok(ExitCode::SUCCESS)
            }
            Err(e) => {
                eprintln!("{}: {e}", "error".red().bold());
                Ok(ExitCode::from(1))
            }
        }
    })
}
