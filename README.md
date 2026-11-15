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

## Source-free toolchain doctor preview

Run `python tools/atto.py doctor` to inspect tool availability on PATH, or `python tools/atto.py doctor --json --require rust --require wasm` to require a selected backend's tools. Missing required tools return exit code 2; output uses schema version 1 and does not expose executable paths. The command never runs detected binaries, invokes a compiler, or makes a runtime/parity claim. Build, run, test, and permission management remain unimplemented here.

Use `python -m unittest discover -s tests -v` for this preview's deterministic host-only tests.
