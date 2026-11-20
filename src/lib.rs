//! Atto command-line runtime policy for the currently executable scalar Wasm host.
//! No compiler or native actor backend is silently promoted by this CLI.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThreadPolicy {
    Default,
    SingleThreaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub wasm: PathBuf,
    pub wasm_runner: PathBuf,
    pub expected_sha256: String,
    pub timeout_ms: Option<u32>,
    pub thread_policy: ThreadPolicy,
}

fn error(code: &str, reason: &str) -> String {
    format!("{code}: {reason}")
}

// Flags, command, and positional arguments now have a single parser authority.
pub mod permissions;
pub mod flags_cli;
pub use flags_cli::{parse_cli_args, parse_run_args};

impl RunConfig {
    /// Arguments are encoded separately, never interpolated into a shell.
    pub fn node_args(&self) -> Vec<OsString> {
        let mut args = vec![
            self.wasm_runner.as_os_str().to_owned(),
            OsString::from(format!("--wasm={}", self.wasm.display())),
            OsString::from(format!("--sha256={}", self.expected_sha256)),
        ];
        if let Some(ms) = self.timeout_ms {
            args.push(OsString::from(format!("--timeout-ms={ms}")));
        }
        if self.thread_policy == ThreadPolicy::SingleThreaded {
            args.push(OsString::from("--single-threaded"));
        }
        args
    }
}

/// Runs only the explicitly selected trusted host script and pinned guest.
/// The host itself enforces private Wasm memory and verifies SHA-256.
pub fn execute_run(config: &RunConfig) -> Result<i32, String> {
    // Public library callers can construct RunConfig directly without using
    // parse_run_args. The execution boundary must validate it independently.
    if !config.wasm.is_absolute()
        || config.wasm.extension() != Some(OsStr::new("wasm"))
    {
        return Err(error("E_ABSOLUTE_PATH_REQUIRED", "invalid absolute .wasm path"));
    }
    if !config.wasm_runner.is_absolute() {
        return Err(error("E_ABSOLUTE_PATH_REQUIRED", "invalid absolute runner path"));
    }
    if config.expected_sha256.len() != 64
        || !config.expected_sha256.bytes().all(|b|
            b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(error("E_INVALID_SHA256", "invalid guest artifact hash"));
    }
    if config.timeout_ms.is_some_and(|v| !(1..=30_000).contains(&v)) {
        return Err(error("E_INVALID_TIMEOUT", "invalid guest execution deadline"));
    }
    // No follow-on executable permissions for a symlink or special file.
    // Node's guest Wasm input uses its own no-follow, single-fd snapshot gate.
    let metadata = std::fs::symlink_metadata(&config.wasm_runner)
        .map_err(|_| error("E_RUNNER_UNAVAILABLE", "trusted runner script not found"))?;
    if !metadata.file_type().is_file() {
        return Err(error("E_RUNNER_UNAVAILABLE", "runner must be a regular file"));
    }
    let mut cmd = Command::new("node");
    cmd.args(config.node_args());
    // Prevent NODE_OPTIONS/--import hooks, cloud tokens or language package
    // credentials leaking into the trusted guest host process.
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    // Windows needs these OS runtime variables to launch Node correctly;
    // do not inherit arbitrary environment variables or NODE_OPTIONS.
    #[cfg(windows)]
    for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    let result = cmd.status().map_err(|e| error(
        "E_HOST_RUNTIME_UNAVAILABLE", &format!("cannot execute Node: {e}")))?;
    Ok(result.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(flags: &[&str]) -> Result<RunConfig, String> {
        let mut args = vec![
            std::env::temp_dir().join("app.wasm").to_string_lossy().into_owned(),
            format!("--runner={}",std::env::temp_dir().join("run-guest.mjs").display()),
            format!("--sha256={}", "a".repeat(64)),
        ];
        args.extend(flags.iter().map(|s| (*s).to_owned()));
        parse_run_args(&args)
    }

    #[test]
    fn all_three_public_spelling_forms_normalize() {
        for flag in ["--single-threaded", "--max-threads-per-space=1",
                     "--mode=single-threaded"] {
            let config = parse(&[flag]).unwrap();
            assert_eq!(config.thread_policy, ThreadPolicy::SingleThreaded);
            let args = config.node_args();
            assert_eq!(args.last().unwrap().to_str(), Some("--single-threaded"));
        }
        assert_eq!(parse(&[
            "--single-threaded", "--max-threads-per-space=1", "--mode=single-threaded"
        ]).unwrap().thread_policy, ThreadPolicy::SingleThreaded);
    }

    #[test]
    fn default_mode_is_not_falsely_claimed_single() {
        let config = parse(&[]).unwrap();
        assert_eq!(config.thread_policy, ThreadPolicy::Default);
        assert!(!config.node_args().iter().any(|s| s.to_str() == Some("--single-threaded")));
    }

    #[test]
    fn rejects_unknown_duplicate_conflicting_and_malformed_flags() {
        for bad in [
            "--single", "--mode=max1", "--mode=trace", "--mode=",
            "--mode=single-threaded,trace", "--mode=,single-threaded",
            "--mode=single-threaded,", "--mode=Single-Threaded",
            "--max-threads-per-space=0", "--max-threads-per-space=01",
            "--max-threads-per-space=+1", "--max-threads-per-space=-1",
            "--max-threads-per-space=1.0", "--max-threads-per-space=1e0",
            "--max-threads-per-space=2",
            "--max-threads-per-space=18446744073709551616",
            "--timeout-ms=0", "--timeout-ms=30001", "--unknown",
            "--single-threaded=false", "--backend=java"
        ] {
            assert!(parse(&[bad]).is_err(), "accepted {bad}");
        }
        assert!(parse(&["--single-threaded", "--max-threads-per-space=2"])
            .unwrap_err().contains("E_CONFLICTING_THREAD_POLICY"));
        assert!(parse(&["--single-threaded", "--single-threaded"]).is_err());
        assert!(parse(&["--max-threads-per-space=1", "--max-threads-per-space=2"])
            .unwrap_err().contains("E_CONFLICTING_THREAD_POLICY"));
        assert!(parse(&["--mode=single-threaded,")
            .unwrap_err().contains("E_EMPTY_MODE"));
        assert!(parse(&["--mode=single-threaded", "--mode=single-threaded"])
            .unwrap_err().contains("E_DUPLICATE_MODE"));
    }

    #[test]
    fn source_and_unpinned_artifacts_reject_without_execution() {
        let source = vec!["--single-threaded".to_owned(), "app.atto".to_owned()];
        assert!(parse_run_args(&source).unwrap_err()
            .contains("E_SOURCE_RUNTIME_UNSUPPORTED"));
        let no_pin = vec![
            std::env::temp_dir().join("app.wasm").to_string_lossy().into_owned(),
            format!("--runner={}", std::env::temp_dir().join("run-guest.mjs").display())
        ];
        assert!(parse_run_args(&no_pin).unwrap_err().contains("E_SHA256_REQUIRED"));
        assert!(parse(&["--", "--debug"]).is_err());
        assert!(parse(&["/tmp/second.wasm"]).is_err());
    }

    #[test]
    fn forged_library_config_is_refused_at_execution_boundary() {
        let mut config = parse(&[]).unwrap();
        config.expected_sha256 = "not-a-digest".to_owned();
        assert!(execute_run(&config).unwrap_err().contains("E_INVALID_SHA256"));
        config.expected_sha256 = "a".repeat(64);
        config.wasm = PathBuf::from("relative.wasm");
        assert!(execute_run(&config).unwrap_err().contains("E_ABSOLUTE_PATH_REQUIRED"));
        config.wasm = std::env::temp_dir().join("app.wasm");
        config.timeout_ms = Some(30_001);
        assert!(execute_run(&config).unwrap_err().contains("E_INVALID_TIMEOUT"));
    }

    #[test]
    fn node_launch_arguments_are_explicitly_bounded() {
        let config = parse(&["--timeout-ms=2500", "--single-threaded"]).unwrap();
        let args = config.node_args();
        assert_eq!(args.len(), 5);
        assert_eq!(args[3].to_str(), Some("--timeout-ms=2500"));
        assert_eq!(args[4].to_str(), Some("--single-threaded"));
    }
}
