#!/usr/bin/env bash
# Run on macOS with a warm ABB build to reproduce 137s vs 0.29s class timings.
set -euo pipefail
REPO="${1:-$(cd "$(dirname "$0")/../../../.." && pwd)}"
cd "$REPO"
PKG=audiobook-boss
FILTER=metadata_intent_validation_contract
OUT="${REPO}/experiments/proof-ab-nextest/results-real"
mkdir -p "$OUT"

run() {
  local id="$1"; shift
  echo "=== $id ==="
  echo "cmd: $*"
  local start end
  start=$(date +%s)
  bash -lc "$*" >"$OUT/${id}.log" 2>&1
  end=$(date +%s)
  echo "elapsed=$((end-start))s"
  rg -c '^     Running ' "$OUT/${id}.log" || true
  rg -c 'running 0 tests' "$OUT/${id}.log" || true
}

cargo test -p "$PKG" --no-run

run cargo-no-target "cargo test -p $PKG $FILTER"
run cargo-lib "cargo test -p $PKG --lib $FILTER"
run cargo-contract-route "cargo test -p $PKG contract_tests"
run cargo-contract-lib "cargo test -p $PKG --lib contract_tests"
run nextest-filter "cargo nextest run -p $PKG -E 'test($FILTER)'"
run nextest-lib "cargo nextest run -p $PKG --lib -E 'test($FILTER)'"

echo "Logs in $OUT"
