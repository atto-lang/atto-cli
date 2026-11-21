//! Opt-in, source-to-independent-Rust borrow verification for trusted CI.
//!
//! This is not normal compiler borrow admission: the compiler continues to
//! check within its own pass. The exporter must first check actual Atto source
//! and emit the bounded typed loan CFG. The independent Rust verifier consumes
//! the manifest over stdin. No shell or shared temporary manifest is used.
//!
//! The present ATTO_LOAN_CFG_V1 profile is a restricted, diagnostic-only SSA
//! proof, not a complete lifetime/exception/actor or guest-emission certificate.

use clap::Args;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};

const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Args)]
pub(crate) struct BorrowCheckArgs {
    /// AttoLang source to compile and recheck independently.
    #[arg(value_name = "SOURCE")]
    source: PathBuf,

    /// Compiler-owned typed CFG exporter (currently experimental).
    #[arg(long, value_name = "PROGRAM", default_value = "atto-loan-cfg-export")]
    exporter: OsString,

    /// Canonical Rust CFG/SSA verifier (currently experimental).
    #[arg(long, value_name = "PROGRAM", default_value = "atto-cfg-ssa-check")]
    checker: OsString,
}

fn classify_status(status: ExitStatus) -> ExitCode {
    // Exit 1: genuine compiler/static-analysis rejection.
    // Exit 2: missing infrastructure, signal or unexpected exit.
    match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(1) => ExitCode::FAILURE,
        _ => ExitCode::from(2),
    }
}

pub(crate) fn run(args: BorrowCheckArgs) -> ExitCode {
    let mut exporter = match Command::new(&args.exporter)
        .arg(&args.source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!(
                "atto borrow-check: cannot start compiler CFG exporter {:?}: {error}",
                args.exporter
            );
            eprintln!("install atto-loan-cfg-export or provide --exporter");
            return ExitCode::from(2);
        }
    };

    let Some(mut stdout) = exporter.stdout.take() else {
        let _ = exporter.kill();
        let _ = exporter.wait();
        eprintln!("atto borrow-check: compiler exporter stdout was not piped");
        return ExitCode::from(2);
    };

    // Limit memory before the checker is invoked. On overflow, kill the
    // producer and never pass a truncated manifest to the verifier.
    let mut manifest = Vec::new();
    let read_result = stdout
        .by_ref()
        .take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut manifest);
    drop(stdout);
    if let Err(error) = read_result {
        manifest.fill(0);
        let _ = exporter.kill();
        let _ = exporter.wait();
        eprintln!("atto borrow-check: cannot read compiler MIR: {error}");
        return ExitCode::from(2);
    }
    if manifest.len() > MAX_MANIFEST_BYTES {
        manifest.fill(0);
        let _ = exporter.kill();
        let _ = exporter.wait();
        eprintln!("atto borrow-check: compiler MIR exceeds 8 MiB limit");
        return ExitCode::from(2);
    }

    let export_status = match exporter.wait() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("atto borrow-check: cannot await compiler exporter: {error}");
            return ExitCode::from(2);
        }
    };
    if !export_status.success() {
        eprintln!("atto borrow-check: compiler-side check or export failed");
        return classify_status(export_status);
    }
    if manifest.is_empty() || std::str::from_utf8(&manifest).is_err() {
        eprintln!("atto borrow-check: compiler exported empty or non-UTF8 MIR");
        return ExitCode::from(2);
    }

    let mut checker = match Command::new(&args.checker)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!(
                "atto borrow-check: cannot start canonical Rust checker {:?}: {error}",
                args.checker
            );
            eprintln!("install atto-cfg-ssa-check or provide --checker");
            return ExitCode::from(2);
        }
    };
    let write_result = checker
        .stdin
        .take()
        .expect("canonical verifier was started with piped stdin")
        .write_all(&manifest);
    // Overwrite source-derived bytes in memory; never create a temp file.
    manifest.fill(0);
    if let Err(error) = write_result {
        let _ = checker.kill();
        let _ = checker.wait();
        eprintln!("atto borrow-check: cannot deliver typed MIR to canonical verifier: {error}");
        return ExitCode::from(2);
    }
    match checker.wait() {
        Ok(status) => classify_status(status),
        Err(error) => {
            eprintln!("atto borrow-check: cannot await canonical verifier: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: BorrowCheckArgs,
    }

    #[test]
    fn defaults_to_two_independent_executables() {
        let parsed = TestCli::try_parse_from(["atto", "demo.atto"]).unwrap();
        assert_eq!(parsed.args.source, PathBuf::from("demo.atto"));
        assert_eq!(parsed.args.exporter, OsString::from("atto-loan-cfg-export"));
        assert_eq!(parsed.args.checker, OsString::from("atto-cfg-ssa-check"));
    }

    #[test]
    fn preserves_exact_executable_overrides_without_shell_interpolation() {
        let parsed = TestCli::try_parse_from([
            "atto",
            "--exporter",
            "/private/compiler export",
            "--checker",
            "/private/borrow checker",
            "demo.atto",
        ])
        .unwrap();
        assert_eq!(parsed.args.exporter, OsString::from("/private/compiler export"));
        assert_eq!(parsed.args.checker, OsString::from("/private/borrow checker"));
    }

    #[test]
    fn missing_source_is_invalid() {
        assert!(TestCli::try_parse_from(["atto"]).is_err());
    }
}
