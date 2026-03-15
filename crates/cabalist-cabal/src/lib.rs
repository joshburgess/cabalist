//! # cabalist-cabal
//!
//! Invokes the `cabal` CLI as a subprocess, streams output, and parses results
//! into structured data. Handles build invocation, dry-run solver plan parsing,
//! GHC diagnostic extraction, and `cabal`/`ghc` version detection.

pub mod build;
pub mod detect;
pub mod diagnostics;
pub mod error;
pub mod invoke;
pub mod solver;

pub use build::{
    cabal_build, cabal_build_dry_run, cabal_clean, cabal_run, cabal_test, BuildOptions,
    BuildResult, TestResult,
};
pub use detect::{detect_toolchain, ToolchainInfo};
pub use diagnostics::{parse_diagnostics, GhcDiagnostic, GhcSeverity};
pub use error::CabalError;
pub use invoke::{CabalOutput, OutputLine};
pub use solver::{parse_plan_json_content, PlannedPackage, SolverPlan};
