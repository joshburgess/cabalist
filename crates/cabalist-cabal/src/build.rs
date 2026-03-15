//! High-level wrappers for `cabal build`, `cabal test`, `cabal clean`, and `cabal run`.

use crate::diagnostics::{parse_diagnostics, GhcDiagnostic};
use crate::error::CabalError;
use crate::invoke::{run_cabal, run_cabal_simple, CabalInvocation, CabalOutput, OutputLine};
use crate::solver::{parse_plan_json_content, SolverPlan};
use std::path::Path;
use tokio::sync::mpsc;

/// Options for a build or test command.
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Specific components to build (empty means build all).
    pub components: Vec<String>,
    /// Extra flags to pass to cabal.
    pub extra_flags: Vec<String>,
}

/// Result of a `cabal build` invocation.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// The raw subprocess output.
    pub output: CabalOutput,
    /// Whether the build succeeded (exit code 0).
    pub success: bool,
    /// Parsed GHC diagnostics from stderr.
    pub diagnostics: Vec<GhcDiagnostic>,
}

/// Result of a `cabal test` invocation.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// The raw subprocess output.
    pub output: CabalOutput,
    /// Whether the tests passed (exit code 0).
    pub success: bool,
    /// Parsed GHC diagnostics from stderr.
    pub diagnostics: Vec<GhcDiagnostic>,
}

/// Run `cabal build` with the given options.
///
/// If `line_sender` is provided, build output is streamed line-by-line for
/// real-time display in the TUI.
pub async fn cabal_build(
    working_dir: &Path,
    opts: &BuildOptions,
    line_sender: Option<mpsc::UnboundedSender<OutputLine>>,
) -> Result<BuildResult, CabalError> {
    let mut args = vec!["build".to_string()];
    args.extend(opts.components.iter().cloned());
    args.extend(opts.extra_flags.iter().cloned());

    let invocation = CabalInvocation {
        args,
        working_dir: working_dir.to_path_buf(),
        env_overrides: Vec::new(),
        timeout: None,
    };

    let output = run_cabal(&invocation, line_sender).await?;
    let diagnostics = parse_diagnostics(&output.stderr);
    let success = output.exit_code == 0;

    Ok(BuildResult {
        output,
        success,
        diagnostics,
    })
}

/// Run `cabal build --dry-run` and parse the resulting solver plan.
///
/// This reads the `plan.json` file produced by the dry-run from the
/// `dist-newstyle/cache/` directory.
pub async fn cabal_build_dry_run(working_dir: &Path) -> Result<SolverPlan, CabalError> {
    let output = run_cabal_simple(&["build", "--dry-run"], working_dir).await?;

    if output.exit_code != 0 {
        return Err(CabalError::CommandFailed {
            exit_code: output.exit_code,
            stderr: output.stderr,
        });
    }

    // cabal build --dry-run writes plan.json to dist-newstyle/cache/plan.json.
    let plan_path = working_dir.join("dist-newstyle/cache/plan.json");

    if plan_path.is_file() {
        let content = std::fs::read_to_string(&plan_path)?;
        parse_plan_json_content(&content)
    } else {
        // Some cabal versions may output the plan to stdout as JSON.
        // Try parsing stdout as a fallback.
        parse_plan_json_content(&output.stdout)
    }
}

/// Run `cabal test` with the given options.
///
/// If `line_sender` is provided, test output is streamed line-by-line.
pub async fn cabal_test(
    working_dir: &Path,
    opts: &BuildOptions,
    line_sender: Option<mpsc::UnboundedSender<OutputLine>>,
) -> Result<TestResult, CabalError> {
    let mut args = vec!["test".to_string()];
    args.extend(opts.components.iter().cloned());
    args.extend(opts.extra_flags.iter().cloned());

    let invocation = CabalInvocation {
        args,
        working_dir: working_dir.to_path_buf(),
        env_overrides: Vec::new(),
        timeout: None,
    };

    let output = run_cabal(&invocation, line_sender).await?;
    let diagnostics = parse_diagnostics(&output.stderr);
    let success = output.exit_code == 0;

    Ok(TestResult {
        output,
        success,
        diagnostics,
    })
}

/// Run `cabal clean` to remove build artifacts.
pub async fn cabal_clean(working_dir: &Path) -> Result<(), CabalError> {
    let output = run_cabal_simple(&["clean"], working_dir).await?;

    if output.exit_code != 0 {
        return Err(CabalError::CommandFailed {
            exit_code: output.exit_code,
            stderr: output.stderr,
        });
    }

    Ok(())
}

/// Run `cabal run <executable>` with optional arguments.
///
/// If `line_sender` is provided, output is streamed line-by-line.
pub async fn cabal_run(
    working_dir: &Path,
    executable: &str,
    args: &[&str],
    line_sender: Option<mpsc::UnboundedSender<OutputLine>>,
) -> Result<CabalOutput, CabalError> {
    let mut cmd_args = vec!["run".to_string(), executable.to_string(), "--".to_string()];
    cmd_args.extend(args.iter().map(|s| s.to_string()));

    let invocation = CabalInvocation {
        args: cmd_args,
        working_dir: working_dir.to_path_buf(),
        env_overrides: Vec::new(),
        timeout: None,
    };

    run_cabal(&invocation, line_sender).await
}
