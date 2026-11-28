> **AI agent instructions (organization-wide):** Before working on code, issues, or pull requests, read and follow the [organization `AGENTS.md`](https://github.com/atto-lang/.github/blob/main/AGENTS.md), including its **merge-first stealth-development policy**. Honor any repository-local `AGENTS.md`, explicit holds, and GitHub-enforced rules.

# atto-cli

Command-line tools for building, running, testing, and managing AttoLang projects.

## Naming migration and language authority

This repository is maintained at [atto-lang/atto-cli](https://github.com/atto-lang/atto-cli); use `https://github.com/atto-lang/atto-cli.git` for Git remotes and canonical `.atto` source examples. The normative language contract is [atto-spec](https://github.com/atto-lang/atto-spec), with executable guest evidence tracked by [atto-platform-conformance](https://github.com/atto-lang/atto-platform-conformance). Follow current ownership, borrowing, effects, actor, and syntax rules there; do not infer implemented features from a README or unexecuted CI.

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

AttoLang is the canonical project name; new source, examples and documentation use `.atto` and the `atto-lang` repository organization. This README defines scope, not feature readiness.

**Acceptance:** name the exact source SHA/tree, runtime/toolchain, positive and negative checks and execution receipt before claiming support. Host-only tests and empty CI steps do not prove guest execution.

## Optional canonical Rust borrow verification (draft)

Normal `atto check program.atto` continues to rely on each compiler's own fast, in-pass ownership/borrow analysis. The new **opt-in** `atto borrow-check program.atto` command launches the compiler's `atto-loan-cfg-export` tool, requires it to successfully compile and export a bounded typed CFG, and passes that manifest over stdin to the independent canonical Rust `atto-cfg-ssa-check` verifier. Both executables are experimental in [atto-source.rs #295](https://github.com/atto-lang/atto-source.rs/pull/295) and [atto-borrow-checker.rs #42](https://github.com/atto-lang/atto-borrow-checker.rs/pull/42); they are **not yet bundled with this CLI**.

The currently supported deep-analysis profile is a limited diagnostic CFG/SSA loan oracle, not proof of whole-program lifetimes, native guest execution or production backend parity. Missing tools or unverified checks fail closed. See [deep borrow verification and CI usage](docs/deep-borrow-check.md).

## Source-free toolchain doctor preview

Run `python tools/atto.py doctor` to inspect tool availability on PATH, or `python tools/atto.py doctor --json --require rust --require wasm` to require a selected backend's tools. Missing required tools return exit code 2; output uses schema version 1 and does not expose executable paths. The command never runs detected binaries, invokes a compiler, or makes a runtime/parity claim. Build, run, test, and permission management remain unimplemented here.

Use `python -m unittest discover -s tests -v` for this preview's deterministic host-only tests.

## Legacy provenance and transfer verification

The historical predecessor was `ores-truffle-oreslang/oreslang-cli`. This repository is maintained in `atto-lang`; matching names or imported history do not establish preserved numeric repository identity or code parity. Preserve working legacy `.ores` inputs and historical package/CLI aliases until compiler-backed replacements and consumers are verified. New source follows canonical `.atto` contracts.

Language build/check/execute developer CLI and compiler invocation contracts belong here; production deployment orchestration and billing do not. [atto-lang](https://github.com/atto-lang) maintains language, portable libraries and tooling; [atto-co](https://github.com/atto-co) separately governs commercial deployment, tenant infrastructure, identity, marketplace and billing. Any further original-ID transfer or commercial carve-out requires separate owner authorization and independent security review. This document authorizes no transfer or private-source mirror.

Before an authorized transfer, coordinate in [migration issue #26](https://github.com/ores-truffle-oreslang/.github/issues/26) and review the authorized runbook. Record source/target administrators, numeric ID, exact SHA/tree, history, visibility, access, rulesets, open PRs/issues, releases, packages, consumers, CI/billing/runners, secrets, deploy keys, apps, webhooks, Pages and rollback ownership. Verify unchanged ID/history/tree and restored controls afterward; execute exact-head tests before changing consumers. Zero-step jobs are NOT_EXECUTED; host-only and source-free checks do not certify compiled guest execution or production capabilities.
