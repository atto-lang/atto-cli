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

## Source-free toolchain doctor preview

Run `python tools/atto.py doctor` to inspect tool availability on PATH, or `python tools/atto.py doctor --json --require rust --require wasm` to require a selected backend's tools. Missing required tools return exit code 2; output uses schema version 1 and does not expose executable paths. The command never runs detected binaries, invokes a compiler, or makes a runtime/parity claim. Build, run, test, and permission management remain unimplemented here.

Use `python -m unittest discover -s tests -v` for this preview's deterministic host-only tests.
