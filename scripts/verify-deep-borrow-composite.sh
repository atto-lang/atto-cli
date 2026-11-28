#!/usr/bin/env bash
# Exact-source, real-compiler → canonical-Rust-SSA smoke.
# Requires three separately checked-out, PINNED private repositories.
# Never uses cross-repository GitHub tokens or downloads private source.
set -euo pipefail

: "${ATTO_CLI_DIR:?set ATTO_CLI_DIR to the checked-out atto-cli repository}"
: "${ATTO_COMPILER_DIR:?set ATTO_COMPILER_DIR to the checked-out atto-source.rs repository}"
: "${ATTO_BORROW_DIR:?set ATTO_BORROW_DIR to the checked-out atto-borrow-checker.rs repository}"
: "${EXPECTED_CLI_SHA:?pin the exact CLI Git commit}"
: "${EXPECTED_COMPILER_SHA:?pin the exact compiler Git commit}"
: "${EXPECTED_BORROW_SHA:?pin the exact canonical checker Git commit}"

verify_repo() {
  local repo="$1" expected="$2" label="$3"
  if [[ ! "$expected" =~ ^[0-9a-f]{40}$ ]]; then
    echo "BAD_EXPECTED_SHA $label" >&2
    exit 2
  fi
  local actual
  actual="$(git -C "$repo" rev-parse HEAD)"
  if [[ "$actual" != "$expected" ]]; then
    echo "SHA_MISMATCH $label expected=$expected actual=$actual" >&2
    exit 2
  fi
  if [[ -n "$(git -C "$repo" status --porcelain --untracked-files=all)" ]]; then
    echo "DIRTY_SOURCE_TREE $label" >&2
    exit 2
  fi
  echo "EXACT_COMMIT $label $actual"
  git -C "$repo" rev-parse 'HEAD^{tree}' | sed "s/^/EXACT_ROOT_TREE $label /"
}

verify_repo "$ATTO_CLI_DIR" "$EXPECTED_CLI_SHA" cli
verify_repo "$ATTO_COMPILER_DIR" "$EXPECTED_COMPILER_SHA" compiler
verify_repo "$ATTO_BORROW_DIR" "$EXPECTED_BORROW_SHA" canonical

echo "BUILDING_ACTUAL_CLI_EXPORTER_AND_CHECKER"
cargo build --locked --manifest-path "$ATTO_CLI_DIR/Cargo.toml" -p oreslang-cli --bin atto
cargo build --locked --manifest-path "$ATTO_COMPILER_DIR/Cargo.toml" --bin atto-loan-cfg-export
cargo build --locked --manifest-path "$ATTO_BORROW_DIR/Cargo.toml" --bin atto-cfg-ssa-check

cli="$ATTO_CLI_DIR/target/debug/atto"
exporter="$ATTO_COMPILER_DIR/target/debug/atto-loan-cfg-export"
checker="$ATTO_BORROW_DIR/target/debug/atto-cfg-ssa-check"
for exe in "$cli" "$exporter" "$checker"; do
  test -x "$exe" || { echo "MISSING_EXECUTABLE $exe" >&2; exit 2; }
done

test_dir="$(mktemp -d)"
trap 'rm -rf "$test_dir"' EXIT
cat >"$test_dir/pass.atto" <<'ATTO'
pub fnc main(): int {
  let number = 40;
  val reader = borrow! number;
  val result = copy! reader;
  number = 42;
  return number;
}
ATTO

# Must invoke the REAL compiler exporter and canonical Rust SSA analyzer.
echo "POSITIVE_COMPILER_TO_CANONICAL"
"$cli" borrow-check "$test_dir/pass.atto" --exporter "$exporter" --checker "$checker" \
  >"$test_dir/pass.out" 2>"$test_dir/pass.err" || {
    cat "$test_dir/pass.err" >&2
    echo "COMPILER_RUST_HANDOFF_FAILED" >&2
    exit 1
  }
if ! grep -q '^SSA_DIAGNOSTIC_OK ' "$test_dir/pass.out"; then
  echo "NO_ACTUAL_RUST_SSA_FUNCTION_PROOF" >&2
  exit 1
fi

# Existing negative fixture was compiler-rejected in the producer PR tests.
negative="$ATTO_COMPILER_DIR/tests/fixtures/fail/ownership-borrow-nll-live-after-mutation.atto"
test -s "$negative" || { echo "MISSING_NEGATIVE_SOURCE" >&2; exit 2; }
echo "NEGATIVE_COMPILER_REJECTION"
set +e
"$cli" borrow-check "$negative" --exporter "$exporter" --checker "$checker" \
    >"$test_dir/fail.out" 2>"$test_dir/fail.err"
negative_status=$?
set -e
if [[ "$negative_status" -ne 1 ]]; then
  cat "$test_dir/fail.err" >&2
  echo "UNSAFE_SOURCE_NOT_SEMANTICALLY_REJECTED status=$negative_status" >&2
  exit 1
fi
if grep -q '^SSA_DIAGNOSTIC_OK ' "$test_dir/fail.out"; then
  echo "REJECTED_SOURCE_PRODUCED_RUST_SUCCESS_REPORT" >&2
  exit 1
fi
echo "SOURCE_TO_CANONICAL_RUST_SMOKE=PASS"
echo "GUEST_EMISSION_COMPLETENESS=NOT_VERIFIED"
echo "ALL_BACKEND_PARITY=NOT_VERIFIED"
