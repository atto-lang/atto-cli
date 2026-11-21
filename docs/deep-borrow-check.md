# Opt-in canonical Rust borrow verification

The Atto CLI has **two distinct safety layers**:

- `atto check program.atto` runs the compiler's **own** type/borrow checks on the normal compilation path. It does not launch the independent Rust checker.
- `atto borrow-check program.atto` runs an **additional**, opt-in independent analysis of a typed loan-CFG manifest produced by the compiler. This is useful in CI and audits, not a substitute for the compiler-native checker.

## Proposed end-to-end invocation (experimental)

The initial implementation depends on two still-draft executables:
- compiler exporter `atto-loan-cfg-export` from [atto-source.rs #295](https://github.com/atto-lang/atto-source.rs/pull/295). It first invokes the actual compiler then emits a versioned, restricted `ATTO_LOAN_CFG_V1` text manifest.
- canonical Rust checker `atto-cfg-ssa-check` from [atto-borrow-checker.rs #42](https://github.com/atto-lang/atto-borrow-checker.rs/pull/42). It validates the manifest and calls the independent Rust SSA loan analyzer. The CLI bridge requires the checker to support a single argument `-` to read the manifest from stdin (part of #42).

Example after building/installing those executables:

```sh
atto check program.atto
atto borrow-check program.atto
atto borrow-check program.atto \
  --exporter ./target/exporter/atto-loan-cfg-export \
  --checker ./target/borrow/atto-cfg-ssa-check
```

The CLI starts the exporter with **one source path argument**; exporter must reject uncheckable source and must not emit a success manifest on failed compilation. Only after a zero-status exporter and nonempty UTF-8 output does the CLI start the checker with `-` and pipe the output into its stdin. No shell interpolation, no token or GitHub credentials, and no temporary manifest file. The exporter/consumer outputs are bounded to **8 MiB**. The source file's bytes and manifest provenance are not cryptographically attested by this command alone.

Exit-code contract:
- `0`: the independent verifier accepted this manifest for its **supported restricted feature profile**. This is **NOT** guest/runtime certification.
- `1`: compiler-native rejection or canonical loan/SSA rejection.
- `2`: exporter/checker unavailable, malformed/too-large output, signal, unexpected exit or I/O failure. This is **NOT_EXECUTED/INCONCLUSIVE**, never success.

There is **no fallback** to a weaker checker when a specified executable is missing, and a compiler-only success never counts as an independent verification success.

## CI pipeline

1. Check out fixed, exact `atto-cli`, `atto-source.rs` and `atto-borrow-checker.rs` heads; verify individual trees and compiler/checker executable hashes.
2. Run the real `atto check`, then the deep `atto borrow-check` on the same pinned positive/negative `.atto` fixtures.
3. Confirm the compiler's own verdict matches independent Rust analysis where both support the same source/MIR profile. A compiler rejection must not be masked by an exporter skipping a bad function or emitting an empty manifest.
4. Prove source-to-MIR and MIR-to-guest instruction census completeness separately. Both tools rely on compiler-submitted typed facts, so their agreement alone is **not an independent emitted-code completeness proof**.
5. Treat a missing runner, `steps=[]`, absent dependency, stale SHA, unsupported feature or unexecuted guest as **NOT VERIFIED**. Do not turn these into green required checks.
6. Only after real compiler/Cargo/guest test receipts, promote to a protected CI gate and consider enabling verified simple-profile caching.

The CLI itself does **not** link to the standalone library. Executing a single Rust verifier process in explicit CI mode is portable across the independently versioned compiler implementations. A future integrated Rust-host configuration may link the canonical crate directly through a pinned, reviewed dependency, but does not need to serialize/reparse per statement or inject a checker into normal compilation.

This first bridge supports only the existing bounded CFG/SSA verification profile. Future cyclic/Polonius/NLL, exception/cancel, pinned-suspension, actor domain and whole-project analyses must remain separately gated until implemented and proven in both the Rust checker and the compiler exporters.
