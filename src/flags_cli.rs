//! Atto run flags are parsed exclusively by the audited bundled flags2env
//! structured parser. This module only validates typed semantic constraints.
use crate::{RunConfig, ThreadPolicy};
use crate::permissions::PermissionRequest;
use flags2env::BundledFlags2Env;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::PathBuf;

fn fail(code: &'static str, reason: &'static str) -> String {
    format!("{code}: {reason}")
}
fn flag<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key).map(String::as_str)
}
fn bool_on(map: &HashMap<String, String>, key: &str) -> Result<bool, String> {
    match flag(map, key) {
        None => Ok(false), Some("true" | "1") => Ok(true),
        _ => Err(fail("E_INVALID_FLAG", "invalid boolean option")),
    }
}

/// This is a *duplicate rejection guard*, never an independent option parser.
/// All option meanings and command/positional interpretation come from flags2env.
fn reject_repeated(argv: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for token in argv {
        if let Some(option) = token.strip_prefix("--") {
            let name = option.split_once('=').map_or(option, |(n, _)| n);
            if !seen.insert(name) {
                return Err(fail("E_DUPLICATE_FLAG", "duplicate option"));
            }
        }
    }
    Ok(())
}

/// Parse the *whole* Atto invocation: argv excludes argv[0].
/// This function never reads live environment values to determine authority.
pub fn parse_cli_args(args: &[String]) -> Result<RunConfig, String> {
    if args.iter().any(|s| s == "--") {
        return Err(fail("E_UNSUPPORTED_APP_ARGS", "scalar guest arguments are not supported"));
    }
    reject_repeated(args)?;
    let parser = BundledFlags2Env::new();
    let contract = concat!(env!("CARGO_MANIFEST_DIR"), "/.cli-flags.toml");
    parser.audit_config(Some(contract))
        .map_err(|_| fail("E_CLI_SCHEMA", "invalid bundled flags2env schema"))?;
    let mut full = vec!["atto".to_string()];
    full.extend_from_slice(args);
    let parsed = parser.parse_structured(&full, Some(contract))
        .map_err(|_| fail("E_CLI_PARSE", "failed to parse CLI arguments"))?;
    if !parsed.errors.is_empty() || !parsed.unknown_options.is_empty() {
        return Err(fail("E_CLI_PARSE", "unknown or malformed CLI option"));
    }
    if parsed.command != "run" {
        return Err(fail("E_COMMAND_UNSUPPORTED", "only pinned Wasm run is available"));
    }

    let flags = &parsed.provided_flags; // argv provenance ONLY.
    let policy = PermissionRequest::from_argv_only(flags)?;
    if policy.requests_privileged_io() {
        // Existing scalar guest runner is deliberately import-free.
        // It does NOT carry an enforced FS/network/env permission manifest.
        return Err(fail("E_HOST_PERMISSIONS_UNSUPPORTED",
            "scalar Wasm runner cannot broker privileged guest I/O"));
    }
    let single_flag = bool_on(flags, "ATTO_SINGLE_THREADED")?;
    let mode_single = match flag(flags, "ATTO_MODE") {
        None => false,
        Some("single-threaded") => true,
        Some(value) if value.split(',').any(str::is_empty) =>
            return Err(fail("E_EMPTY_MODE", "empty mode element")),
        _ => return Err(fail("E_UNKNOWN_MODE", "unknown execution mode")),
    };
    let thread_limit = match flag(flags, "ATTO_MAX_THREADS_PER_SPACE") {
        None => None,
        Some(raw) => {
            if raw.is_empty() || raw.starts_with('0')
                || !raw.bytes().all(|b| b.is_ascii_digit()) {
                return Err(fail("E_INVALID_THREAD_LIMIT", "invalid thread limit"));
            }
            Some(raw.parse::<u64>()
                .map_err(|_| fail("E_INVALID_THREAD_LIMIT", "overflowed thread limit"))?)
        }
    };
    let requested_single = single_flag || mode_single || thread_limit == Some(1);
    if thread_limit.is_some_and(|limit| limit != 1) {
        return Err(fail(if requested_single {
            "E_CONFLICTING_THREAD_POLICY"
        } else { "E_UNSUPPORTED_THREAD_LIMIT" },
        "only single-threaded guest execution is supported"));
    }
    let thread_policy = if requested_single { ThreadPolicy::SingleThreaded }
        else { ThreadPolicy::Default };

    if flag(flags, "ATTO_BACKEND").is_some_and(|value| value != "wasm") {
        return Err(fail("E_BACKEND_UNSUPPORTED", "only scalar Wasm is wired"));
    }
    let input = match parsed.extras.as_slice() {
        [input] => input,
        [] => return Err(fail("E_INPUT_REQUIRED", "guest file path required")),
        _ => return Err(fail("E_DUPLICATE_INPUT", "multiple program inputs")),
    };
    let wasm = PathBuf::from(input);
    if wasm.extension() != Some(OsStr::new("wasm")) {
        return Err(fail("E_SOURCE_RUNTIME_UNSUPPORTED",
            "compiling and running .atto source is not yet supported"));
    }
    if !wasm.is_absolute() {
        return Err(fail("E_ABSOLUTE_PATH_REQUIRED", "guest must use an absolute path"));
    }
    let wasm_runner = PathBuf::from(flag(flags, "ATTO_RUNNER")
        .ok_or_else(|| fail("E_RUNNER_REQUIRED", "trusted runner path required"))?);
    if !wasm_runner.is_absolute() {
        return Err(fail("E_ABSOLUTE_PATH_REQUIRED", "runner must use an absolute path"));
    }
    let expected_sha256 = flag(flags, "ATTO_SHA256")
        .ok_or_else(|| fail("E_SHA256_REQUIRED", "pinned artifact digest required"))?
        .to_string();
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b|
        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(fail("E_INVALID_SHA256", "requires 64 lowercase hex characters"));
    }
    let timeout_ms = match flag(flags, "ATTO_TIMEOUT_MS") {
        None => None,
        Some(raw) => {
            if raw.is_empty() || raw.starts_with('0')
                || !raw.bytes().all(|b| b.is_ascii_digit()) {
                return Err(fail("E_INVALID_TIMEOUT", "invalid timeout format"));
            }
            let value: u32 = raw.parse().map_err(|_|
                fail("E_INVALID_TIMEOUT", "timeout exceeds u32"))?;
            if !(1..=30_000).contains(&value) {
                return Err(fail("E_INVALID_TIMEOUT", "timeout outside 1..30000"));
            }
            Some(value)
        }
    };
    Ok(RunConfig { wasm, wasm_runner, expected_sha256, timeout_ms, thread_policy })
}

/// Kept for old Rust host-only tests; still uses the SAME flags2env parser.
pub fn parse_run_args(args: &[String]) -> Result<RunConfig, String> {
    let mut full = vec!["run".to_string()];
    full.extend_from_slice(args);
    parse_cli_args(&full)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pin() -> Vec<String> {
        vec!["run".to_string(), format!("--sha256={}", "a".repeat(64)),
             format!("--runner={}", std::env::temp_dir().join("runner.mjs").display()),
             std::env::temp_dir().join("guest.wasm").to_string_lossy().into_owned()]
    }
    #[test]
    fn grant_request_is_rejected_even_with_pinned_guest() {
        for option in ["--allow-read=./data", "--allow-net=example.com",
                       "--allow-all", "--allow-ffi", "--allow-env=SECRET"] {
            let mut args = pin(); args.insert(1, option.to_owned());
            let err = parse_cli_args(&args).unwrap_err();
            assert!(err.contains("E_HOST_PERMISSIONS_UNSUPPORTED"), "{option}: {err}");
        }
    }
    #[test]
    fn explicit_denies_and_no_grants_remain_safe_for_import_free_guest() {
        let mut args = pin();
        args.insert(1, "--deny-net=*".to_owned());
        let parsed = parse_cli_args(&args).unwrap();
        assert_eq!(parsed.thread_policy, ThreadPolicy::Default);
    }
    #[test]
    fn duplicate_scoped_grants_never_last_wins() {
        let mut args = pin();
        args.insert(1, "--allow-net=example.com".into());
        args.insert(2, "--allow-net=other.example.com".into());
        assert!(parse_cli_args(&args).unwrap_err().contains("E_DUPLICATE_FLAG"));
    }
}
