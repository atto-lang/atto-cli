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

/// Parses the CLI in a source-free, strict way before invoking any subprocess.
/// The scalar Wasm profile cannot accept guest command-line arguments.
pub fn parse_run_args(args: &[String]) -> Result<RunConfig, String> {
    let mut source: Option<PathBuf> = None;
    let mut runner: Option<PathBuf> = None;
    let mut sha: Option<String> = None;
    let mut timeout_ms: Option<u32> = None;
    let mut backend: Option<&str> = None;
    let mut single_flag = false;
    let mut mode_single = false;
    let mut numeric: Option<u64> = None;

    for arg in args {
        if arg == "--" {
            return Err(error("E_UNSUPPORTED_APP_ARGS",
                "scalar Wasm guest does not support application arguments"));
        }
        if arg == "--single-threaded" {
            if single_flag {
                return Err(error("E_DUPLICATE_THREAD_FLAG", "repeated --single-threaded"));
            }
            single_flag = true;
        } else if let Some(raw) = arg.strip_prefix("--max-threads-per-space=") {
            if raw.is_empty() || raw.starts_with('0')
                || !raw.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(error("E_INVALID_THREAD_LIMIT",
                    "expected canonical positive decimal integer"));
            }
            let value: u64 = raw.parse().map_err(|_| error(
                "E_INVALID_THREAD_LIMIT", "thread limit overflows u64"))?;
            if numeric.is_some() {
                return Err(error("E_DUPLICATE_THREAD_FLAG",
                    "thread limit repeated"));
            }
            numeric = Some(value);
        } else if let Some(raw) = arg.strip_prefix("--mode=") {
            for mode in raw.split(',') {
                if mode.is_empty() {
                    return Err(error("E_EMPTY_MODE", "empty mode item"));
                }
                if mode != "single-threaded" {
                    return Err(error("E_UNKNOWN_MODE",
                        "scalar Wasm host only supports single-threaded mode"));
                }
                if mode_single {
                    return Err(error("E_DUPLICATE_MODE",
                        "single-threaded mode repeated"));
                }
                mode_single = true;
            }
        } else if let Some(raw) = arg.strip_prefix("--backend=") {
            if backend.replace(raw).is_some() {
                return Err(error("E_DUPLICATE_FLAG", "backend repeated"));
            }
        } else if let Some(raw) = arg.strip_prefix("--runner=") {
            if runner.replace(PathBuf::from(raw)).is_some() {
                return Err(error("E_DUPLICATE_FLAG", "runner repeated"));
            }
        } else if let Some(raw) = arg.strip_prefix("--sha256=") {
            if raw.len() != 64 || !raw.bytes().all(|b|
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(error("E_INVALID_SHA256",
                    "requires 64 lowercase hexadecimal digits"));
            }
            if sha.replace(raw.to_owned()).is_some() {
                return Err(error("E_DUPLICATE_FLAG", "sha256 repeated"));
            }
        } else if let Some(raw) = arg.strip_prefix("--timeout-ms=") {
            if raw.is_empty() || raw.starts_with('0')
                || !raw.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(error("E_INVALID_TIMEOUT", "invalid positive timeout"));
            }
            let value: u32 = raw.parse().map_err(|_| error(
                "E_INVALID_TIMEOUT", "timeout overflow"))?;
            if !(1..=30_000).contains(&value) {
                return Err(error("E_INVALID_TIMEOUT", "expected 1..30000 ms"));
            }
            if timeout_ms.replace(value).is_some() {
                return Err(error("E_DUPLICATE_FLAG", "timeout repeated"));
            }
        } else if arg.starts_with('-') {
            return Err(error("E_UNKNOWN_FLAG", "unknown or unsupported run flag"));
        } else if source.replace(PathBuf::from(arg)).is_some() {
            return Err(error("E_DUPLICATE_INPUT", "multiple program paths"));
        }
    }

    let requested_single = single_flag || mode_single || numeric == Some(1);
    if numeric.is_some_and(|n| n != 1) {
        if requested_single {
            return Err(error("E_CONFLICTING_THREAD_POLICY",
                "single-threaded requested with incompatible limit"));
        }
        return Err(error("E_UNSUPPORTED_THREAD_LIMIT",
            "only max-threads-per-space=1 is supported"));
    }
    let thread_policy = if requested_single {
        ThreadPolicy::SingleThreaded
    } else {
        ThreadPolicy::Default
    };
    if backend.is_some_and(|b| b != "wasm") {
        return Err(error("E_BACKEND_UNSUPPORTED",
            "only scalar Wasm runtime is wired; other backends fail closed"));
    }
    let wasm = source.ok_or_else(|| error("E_INPUT_REQUIRED",
        "provide an absolute .wasm artifact path"))?;
    if wasm.extension() != Some(OsStr::new("wasm")) {
        return Err(error("E_SOURCE_RUNTIME_UNSUPPORTED",
            "source .atto execution is not yet wired; only pinned scalar .wasm is supported"));
    }
    if !wasm.is_absolute() {
        return Err(error("E_ABSOLUTE_PATH_REQUIRED",
            "the Wasm artifact path must be absolute"));
    }
    let wasm_runner = runner.ok_or_else(|| error("E_RUNNER_REQUIRED",
        "provide --runner=/absolute/path/to/atto-source.wasm/scripts/run-guest.mjs"))?;
    if !wasm_runner.is_absolute() {
        return Err(error("E_ABSOLUTE_PATH_REQUIRED",
            "the runner script path must be absolute"));
    }
    let expected_sha256 = sha.ok_or_else(|| error("E_SHA256_REQUIRED",
        "pin the exact guest artifact SHA-256 before execution"))?;
    Ok(RunConfig {
        wasm,
        wasm_runner,
        expected_sha256,
        timeout_ms,
        thread_policy,
    })
}

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
    if !config.wasm_runner.is_file() {
        return Err(error("E_RUNNER_UNAVAILABLE", "trusted runner script not found"));
    }
    let mut cmd = Command::new("node");
    cmd.args(config.node_args());
    // Prevent NODE_OPTIONS/--import hooks, cloud tokens or language package
    // credentials leaking into the trusted guest host process.
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
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
    fn node_launch_arguments_are_explicitly_bounded() {
        let config = parse(&["--timeout-ms=2500", "--single-threaded"]).unwrap();
        let args = config.node_args();
        assert_eq!(args.len(), 5);
        assert_eq!(args[3].to_str(), Some("--timeout-ms=2500"));
        assert_eq!(args[4].to_str(), Some("--single-threaded"));
    }
}
