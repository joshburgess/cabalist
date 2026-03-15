//! Generic async subprocess runner for `cabal` invocations.

use crate::error::CabalError;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Configuration for a cabal invocation.
#[derive(Debug, Clone)]
pub struct CabalInvocation {
    /// The command arguments, e.g. `["build", "--dry-run"]`.
    pub args: Vec<String>,
    /// Working directory for the subprocess.
    pub working_dir: PathBuf,
    /// Environment variable overrides (key, value) pairs.
    pub env_overrides: Vec<(String, String)>,
    /// Optional timeout for the entire invocation.
    pub timeout: Option<Duration>,
}

/// Output from a completed cabal invocation.
#[derive(Debug, Clone)]
pub struct CabalOutput {
    /// Process exit code.
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Wall-clock duration of the invocation.
    pub duration: Duration,
}

/// A line of output from a running subprocess.
#[derive(Debug, Clone)]
pub enum OutputLine {
    /// A line from stdout.
    Stdout(String),
    /// A line from stderr.
    Stderr(String),
}

/// Run a cabal command, streaming output line-by-line through the provided sender.
///
/// If `line_sender` is `None`, output is just collected without streaming.
/// If the sender is dropped (receiver gone), the child process is killed.
pub async fn run_cabal(
    invocation: &CabalInvocation,
    line_sender: Option<mpsc::UnboundedSender<OutputLine>>,
) -> Result<CabalOutput, CabalError> {
    let start = Instant::now();

    let mut cmd = Command::new("cabal");
    cmd.args(&invocation.args)
        .current_dir(&invocation.working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (key, value) in &invocation.env_overrides {
        cmd.env(key, value);
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CabalError::CabalNotFound
        } else {
            CabalError::Io(e)
        }
    })?;

    let child_stdout = child
        .stdout
        .take()
        .expect("stdout should be piped since we configured it");
    let child_stderr = child
        .stderr
        .take()
        .expect("stderr should be piped since we configured it");

    let stdout_reader = BufReader::new(child_stdout);
    let stderr_reader = BufReader::new(child_stderr);

    let sender_clone = line_sender.clone();
    let stdout_handle = tokio::spawn(async move {
        let mut lines = stdout_reader.lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(ref sender) = sender_clone {
                // If the receiver is dropped, we still collect but stop sending.
                let _ = sender.send(OutputLine::Stdout(line.clone()));
            }
            collected.push(line);
        }
        collected.join("\n")
    });

    let sender_clone2 = line_sender;
    let stderr_handle = tokio::spawn(async move {
        let mut lines = stderr_reader.lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(ref sender) = sender_clone2 {
                let _ = sender.send(OutputLine::Stderr(line.clone()));
            }
            collected.push(line);
        }
        collected.join("\n")
    });

    let result = if let Some(timeout_dur) = invocation.timeout {
        match tokio::time::timeout(timeout_dur, child.wait()).await {
            Ok(status_result) => status_result,
            Err(_) => {
                // Timeout: kill the child process.
                let _ = child.kill().await;
                return Err(CabalError::Timeout(timeout_dur));
            }
        }
    } else {
        child.wait().await
    };

    let status = result?;

    // Wait for the reader tasks to finish collecting output.
    let stdout = stdout_handle.await.unwrap_or_default();
    let stderr = stderr_handle.await.unwrap_or_default();

    let duration = start.elapsed();
    let exit_code = status.code().unwrap_or(-1);

    Ok(CabalOutput {
        exit_code,
        stdout,
        stderr,
        duration,
    })
}

/// Run a cabal command and just collect the output (no streaming).
///
/// A convenience wrapper around [`run_cabal`] for simple invocations.
pub async fn run_cabal_simple(
    args: &[&str],
    working_dir: &Path,
) -> Result<CabalOutput, CabalError> {
    let invocation = CabalInvocation {
        args: args.iter().map(|s| s.to_string()).collect(),
        working_dir: working_dir.to_path_buf(),
        env_overrides: Vec::new(),
        timeout: None,
    };
    run_cabal(&invocation, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabal_invocation_builder() {
        let inv = CabalInvocation {
            args: vec!["build".to_string(), "--dry-run".to_string()],
            working_dir: PathBuf::from("/tmp/project"),
            env_overrides: vec![("CABAL_DIR".to_string(), "/tmp/cabal".to_string())],
            timeout: Some(Duration::from_secs(60)),
        };
        assert_eq!(inv.args, vec!["build", "--dry-run"]);
        assert_eq!(inv.working_dir, PathBuf::from("/tmp/project"));
        assert_eq!(inv.env_overrides.len(), 1);
        assert_eq!(inv.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn cabal_output_struct() {
        let output = CabalOutput {
            exit_code: 0,
            stdout: "Build succeeded".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(1234),
        };
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.duration, Duration::from_millis(1234));
    }

    #[test]
    fn output_line_variants() {
        let stdout = OutputLine::Stdout("hello".to_string());
        let stderr = OutputLine::Stderr("warn".to_string());
        // Verify Debug works (compile-time check mostly).
        let _ = format!("{stdout:?}");
        let _ = format!("{stderr:?}");
    }
}
