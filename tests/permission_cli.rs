//! Compiled `atto` Rust executable regression, NOT compiled Atto guest execution.
use std::process::{Command, Output};

fn run(extra: &[&str], env_grant: Option<(&str, &str)>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atto"));
    command.args(["run", "--backend=wasm", "--runner=/tmp/atto-trusted-runner.mjs",
                  "--sha256=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                  "/tmp/atto-nonexistent-guest.wasm"]);
    command.args(extra);
    // The linked subprocess must not derive permission authority from ambient
    // ATTO_ALLOW_ALL, or from the package's .cli-flags.toml default map.
    command.env_remove("ATTO_ALLOW_ALL");
    command.env_remove("ATTO_ALLOW_READ");
    command.env_remove("ATTO_ALLOW_NET");
    if let Some((key, value)) = env_grant { command.env(key, value); }
    command.output().expect("atto CLI executable should start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("diagnostics must be UTF-8")
}

#[test]
fn explicit_guest_grants_fail_before_node_or_any_host_io() {
    for flag in ["--allow-all", "--allow-read=./data",
                 "--allow-net=example.com:443", "--allow-run", "--allow-ffi"] {
        let output = run(&[flag], None);
        assert_eq!(output.status.code(), Some(2), "{flag}: {}", stderr(&output));
        assert!(stderr(&output).contains("E_HOST_PERMISSIONS_UNSUPPORTED"),
                "{flag}: {}", stderr(&output));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn environment_variables_do_not_become_operator_grants() {
    let output = run(&[], Some(("ATTO_ALLOW_ALL", "true")));
    assert!(!stderr(&output).contains("E_HOST_PERMISSIONS_UNSUPPORTED"),
            "ambient env must not supply a grant request");
    // This test provides no real guest/runner, so execution must still fail;
    // this is NOT proof of any successful guest execution.
    assert!(!output.status.success());
}

#[test]
fn malformed_scope_and_duplicate_flag_fail_closed() {
    for flag in ["--allow-read=./data,", "--allow-net=example.com:0",
                 "--allow-read=../secret"] {
        let output = run(&[flag], None);
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("E_PERMISSION_FLAG"),
                "{flag}: {}", stderr(&output));
    }
    let repeated = run(&["--allow-net=example.com", "--allow-net=other.example.com"], None);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(stderr(&repeated).contains("E_DUPLICATE_FLAG"));
}

#[test]
fn deny_only_does_not_bypass_missing_runner() {
    let output = run(&["--deny-net=*"], None);
    assert!(!output.status.success());
    assert!(!stderr(&output).contains("E_HOST_PERMISSIONS_UNSUPPORTED"),
            "deny-only must not be silently reclassified as allow-all");
}

#[test]
fn unknown_and_unsupported_host_cli_flags_fail() {
    let output = run(&["--unrecognized-permission"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("E_CLI_PARSE"));
    let output = run(&["--backend=java"], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("E_BACKEND_UNSUPPORTED"));
}
