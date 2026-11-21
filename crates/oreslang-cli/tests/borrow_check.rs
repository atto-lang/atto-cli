//! CLI transport tests with deliberately minimal stub programs.
//! These prove only bounded process orchestration, NOT Rust loan semantics.
//! Actual compiler-exporter → canonical-Rust checks require a composite CI job.
use std::process::Command;

#[test]
fn missing_exporter_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_atto"))
        .args([
            "borrow-check",
            "--exporter",
            "/deliberately-absent-atto-loan-exporter",
            "hello.atto",
        ])
        .output()
        .expect("Atto CLI executable");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot start compiler CFG exporter"));
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "atto-borrow-bridge-{tag}-{}",
                std::process::id()
            ));
            fs::create_dir(&dir).expect("create exclusive fixture directory");
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .expect("restrict fixture directory");
            Self(dir)
        }

        fn script(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, body).expect("write stub executable");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("restrict stub executable");
            path
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn invoke(exporter: &Path, checker: &Path) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_atto"))
            .args(["borrow-check", "--exporter"])
            .arg(exporter)
            .arg("--checker")
            .arg(checker)
            .arg("guest.atto")
            .output()
            .expect("Atto CLI executable")
    }

    const VALID_EXPORTER: &str =
        "#!/bin/sh\n[ \"$#\" -eq 1 ] || exit 4\nprintf 'ATTO_LOAN_CFG_V1\\n'\n";
    const VALID_CHECKER: &str = concat!(
        "#!/bin/sh\n",
        "[ \"$1\" = '-' ] || exit 4\n",
        "read -r line\n",
        "[ \"$line\" = 'ATTO_LOAN_CFG_V1' ] || exit 5\n",
        "printf 'SSA_DIAGNOSTIC_OK stub 1 1\\n'\n"
    );

    #[test]
    fn pipes_compiler_output_into_checker_stdin() {
        let dir = Sandbox::new("success");
        let exporter = dir.script("exporter with spaces", VALID_EXPORTER);
        let checker = dir.script("checker with spaces", VALID_CHECKER);
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "SSA_DIAGNOSTIC_OK stub 1 1\n");
    }

    #[test]
    fn compiler_rejection_never_invokes_independent_checker() {
        let dir = Sandbox::new("compiler-reject");
        let exporter = dir.script("exporter", "#!/bin/sh\necho E_BORROW >&2\nexit 1\n");
        let checker = dir.script(
            "checker",
            "#!/bin/sh\necho CHECKER_SHOULD_NOT_RUN\nexit 0\n",
        );
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains("CHECKER_SHOULD_NOT_RUN"));
    }

    #[test]
    fn checker_finding_rejects_compilation_claim() {
        let dir = Sandbox::new("checker-reject");
        let exporter = dir.script("exporter", VALID_EXPORTER);
        let checker = dir.script(
            "checker",
            "#!/bin/sh\ncat > /dev/null\necho INDEPENDENT_SSA_REJECT >&2\nexit 1\n",
        );
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("INDEPENDENT_SSA_REJECT"));
    }

    #[test]
    fn missing_checker_is_infrastructure_failure() {
        let dir = Sandbox::new("checker-missing");
        let exporter = dir.script("exporter", VALID_EXPORTER);
        let output = invoke(&exporter, Path::new("/deliberately-absent-canonical-checker"));
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }

    #[test]
    fn malformed_exporter_output_fails_closed() {
        let dir = Sandbox::new("invalid-utf8");
        let exporter = dir.script("exporter", "#!/bin/sh\nprintf '\\377'\n");
        let checker = dir.script("checker", VALID_CHECKER);
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("non-UTF8"));
    }

    #[test]
    fn oversized_export_is_rejected_without_invoking_checker() {
        let dir = Sandbox::new("overflow");
        let exporter = dir.script(
            "exporter",
            "#!/bin/sh\nhead -c 8388609 /dev/zero\n",
        );
        let checker = dir.script(
            "checker",
            "#!/bin/sh\necho CHECKER_SHOULD_NOT_RUN\nexit 0\n",
        );
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds 8 MiB"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("CHECKER_SHOULD_NOT_RUN"));
    }

    #[test]
    fn unexpected_checker_exit_is_inconclusive_not_a_pass() {
        let dir = Sandbox::new("checker-broken");
        let exporter = dir.script("exporter", VALID_EXPORTER);
        let checker = dir.script("checker", "#!/bin/sh\ncat > /dev/null\nexit 7\n");
        let output = invoke(&exporter, &checker);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
}
