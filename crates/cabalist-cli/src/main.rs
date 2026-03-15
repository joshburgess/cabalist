use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod commands;
mod util;

#[derive(Parser)]
#[command(
    name = "cabalist-cli",
    about = "Non-interactive CLI for Haskell .cabal file management"
)]
#[command(version)]
struct Cli {
    /// Path to the .cabal file (auto-detected if not specified)
    #[arg(short, long, global = true)]
    file: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new Haskell project
    Init {
        /// Project name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
        /// Project type
        #[arg(long, default_value = "lib-and-exe")]
        r#type: ProjectType,
        /// License
        #[arg(long, default_value = "MIT")]
        license: String,
        /// Author name
        #[arg(long)]
        author: Option<String>,
        /// Don't create directories, just the .cabal file
        #[arg(long)]
        minimal: bool,
    },
    /// Add a dependency
    Add {
        /// Package name
        package: String,
        /// Version constraint (e.g. "^>=2.0", ">=1.0 && <2.0")
        #[arg(long)]
        version: Option<String>,
        /// Target component (e.g. "library", "exe:my-exe", "test:my-tests")
        #[arg(long, default_value = "library")]
        component: String,
    },
    /// Remove a dependency
    Remove {
        /// Package name
        package: String,
        /// Target component
        #[arg(long, default_value = "library")]
        component: String,
    },
    /// Run opinionated lints
    Check {
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
    },
    /// Format the .cabal file
    Fmt {
        /// Check formatting without modifying (exit 1 if changes needed)
        #[arg(long)]
        check: bool,
    },
    /// Show dependency information
    Deps {
        /// Show as dependency tree
        #[arg(long)]
        tree: bool,
        /// Show outdated packages (requires network)
        #[arg(long)]
        outdated: bool,
    },
    /// List and manage modules
    Modules {
        /// Scan filesystem for .hs files not listed in .cabal
        #[arg(long)]
        scan: bool,
        /// Target component
        #[arg(long, default_value = "library")]
        component: String,
    },
    /// Show project summary
    Info,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ProjectType {
    Library,
    Application,
    LibAndExe,
    Full,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init {
            name,
            r#type,
            license,
            author,
            minimal,
        } => commands::init::run(name, r#type, license, author, minimal),
        Command::Add {
            package,
            version,
            component,
        } => commands::add::run(&cli.file, &package, version.as_deref(), &component),
        Command::Remove { package, component } => {
            commands::remove::run(&cli.file, &package, &component)
        }
        Command::Check { strict } => commands::check::run(&cli.file, strict, cli.format),
        Command::Fmt { check } => commands::fmt::run(&cli.file, check),
        Command::Deps { tree, outdated } => commands::deps::run(&cli.file, tree, outdated),
        Command::Modules { scan, component } => commands::modules::run(&cli.file, scan, &component),
        Command::Info => commands::info::run(&cli.file, cli.format),
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
