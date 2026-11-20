# atto-cli

Command-line tools for building, running, testing, and managing AttoLang projects.

## Purpose

Provide a consistent CLI surface for builds, run/test workflows, and permissions.

## Repository responsibilities

- Keep interfaces, schemas, and behavior documented in a backend-neutral form where applicable.
- Use test fixtures and reproducible evidence to distinguish proposed behavior from implemented behavior.
- Record breaking changes, migration notes, and dependencies on other AttoLang components.

## Validation

Validation should rely on checked-in tests and exact-commit CI results. When reporting parity, distinguish **passed**, **failed**, **unsupported**, and **not executed**. Avoid inferring guest-runtime support from host-only tests.

## Status and development

This README records the intended role of this repository; it is not a claim of feature completeness. Consult the source tree and workflow files for actual implementation and verified build commands. Contributions should preserve compatibility with the canonical language contract.

## Ecosystem

- [AttoLang](https://github.com/atto-lang)
- [Language specification](https://github.com/atto-lang/atto-spec)
- [Cross-platform conformance](https://github.com/atto-lang/atto-platform-conformance)

## Repository ownership and contribution routing

**Belongs here:** Cross-backend user-facing build/run/test entry points, project manifests, target selection and ergonomic diagnostics.

**Belongs elsewhere:** Individual compiler backend implementations belong in atto-source.*; test harness mechanics belong in atto-test.cli; language grammar in atto-spec.

**Security and portability boundary:** Do not quietly enable network/host capabilities. Fail closed on unsupported targets; pin selected compiler, artifact and manifest provenance.

AttoLang is the public name; preserve existing Oreslang/.ores compatibility and use canonical .atto for new portable library source. This README defines scope, not feature readiness.

**Acceptance:** name the exact source SHA/tree, runtime/toolchain, positive and negative checks and execution receipt before claiming support. Host-only tests and empty CI steps do not prove guest execution.

## Source-free toolchain doctor preview

Run `python tools/atto.py doctor` to inspect tool availability on PATH, or `python tools/atto.py doctor --json --require rust --require wasm` to require a selected backend's tools. Missing required tools return exit code 2; output uses schema version 1 and does not expose executable paths. The command never runs detected binaries, invokes a compiler, or makes a runtime/parity claim. Build, run, test, and permission management remain unimplemented here.

Use `python -m unittest discover -s tests -v` for this preview's deterministic host-only tests.


## Rust `atto run` (initial executable Wasm host route)

This new, std-only Rust CLI is **limited to precompiled, SHA-256-pinned,
scalar core-Wasm guests**. It does NOT yet compile or run `.atto` source
and it is not a general Java/LLVM/.NET/Forge scheduler.

```sh
cargo run -- run --single-threaded \
  --backend=wasm \
  --runner=/absolute/path/to/atto-source.wasm/scripts/run-guest.mjs \
  --sha256=<64-lowercase-hex-digits> /absolute/path/to/program.wasm

# Equivalent spelling:
cargo run -- run --mode=single-threaded \
  --runner=/absolute/path/to/run-guest.mjs \
  --sha256=<sha256> /absolute/path/to/program.wasm
```

The Rust frontend validates `--single-threaded`,
`--max-threads-per-space=1`, and `--mode=single-threaded` before execution.
It refuses malformed/unsupported/duplicate flags, unpinned or relative
artifacts, and other backends. It launches an explicitly selected trusted Node
runner with an argument vector (no shell) and a minimized environment.
That runner must be the version from companion
[atto-source.wasm PR #23](https://github.com/atto-lang/atto-source.wasm/pull/23):
it enforces one Worker per guest, import-free private Wasm memory, and
returns a verified admission receipt. Other guest runtimes, guest-level
actor schedulers, source compiler integration and native single-threaded
execution remain unsupported. The preexisting Python doctor is a separate
bootstrap preview, not this Rust `atto run` entrypoint.

Run `cargo test --offline --all-targets` for **source-free** Rust CLI unit
tests. Cross-repo guest execution requires both repositories and an approved
runtime artifact; a passing CLI test is not backend conformance.


## Deno-style permission parsing (stacked draft; NOT guest I/O enforcement)

This branch is stacked on the existing Rust scalar-Wasm launcher PR #4. The
public Rust `atto run` argv parser is migrated to **one pinned bundled
`flags2env`** authority, with `.cli-flags.toml` as its option schema.
`provided_flags` contains operator argv overrides; ambient environment and
`.env` data do **not** create permission grants. Malformed scopes, ambiguous
network endpoints, duplicate/conflicting flags, and invalid UTF-8 argv must
reject. The source tree intentionally retains the old independent Python
`doctor` preview pending a separate Rust migration: that Python command is
**not** the new runtime CLI and cannot grant guest authority.

Supported permission request forms are `--allow/deny-{read,write,net,env,run,sys,ffi,import}[=scope]`,
`--allow-all`, `--no-prompt`, and `--permission-check=runtime|compile`.
The branch does **not** introduce a security sandbox, trusted manifest signer,
filesystem/network host broker, source `.atto` compilation, or general
cross-backend guest execution.

**Current scalar Wasm support is import-free.** Passing any privileged allow
request, even `--allow-net` or `--allow-read=./data`, must return
`E_HOST_PERMISSIONS_UNSUPPORTED` and never silently become a no-op or
unrestricted host capability. Explicit deny-only flags can be accepted for
this no-host-import profile, but they are not evidence of OS sandboxing.
This remains a draft pending real Rust test execution, reviewed dependency
locking, and the runtime host-broker work in atto-cli #6 / conformance #107.
Do not infer that `atto run --allow-read=./data main.atto` works yet.

The dependency is pinned to the public `flags-2-env/flags-2-env` commit
`278245253ee5d37b1fb54285383ab18818de6b18`. A release must additionally
commit/audit a complete `Cargo.lock` and establish a trusted install-time
schema path instead of relying on `CARGO_MANIFEST_DIR`.
