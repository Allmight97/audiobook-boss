#!/usr/bin/env bash
# A/B harness: Cargo libtest vs cargo-nextest on synthetic multi-binary topology.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/synthetic-crate"
OUT="$ROOT/results"
FILTER="metadata_intent_validation_contract"
PKG="audiobook-boss-synthetic"
mkdir -p "$OUT"

log() { printf '%s\n' "$*" | tee -a "$OUT/run.log"; }

count_running_lines() {
  rg -c '^     Running ' "$1" 2>/dev/null || true
}

count_zero_test_bins() {
  rg -c 'running 0 tests' "$1" 2>/dev/null || true
}

run_timed() {
  local id="$1"
  shift
  local logfile="$OUT/${id}.log"
  log ""
  log "=== $id ==="
  log "cmd: $*"
  local start end elapsed
  start="$(date +%s.%N)"
  (cd "$CRATE" && bash -lc "$*") >"$logfile" 2>&1 || true
  end="$(date +%s.%N)"
  elapsed="$(awk -v s="$start" -v e="$end" 'BEGIN { printf "%.3f", e - s }')"
  local running zero passed failed
  running="$(count_running_lines "$logfile")"
  zero="$(count_zero_test_bins "$logfile")"
  passed="$(rg -c 'test result: ok\.' "$logfile" 2>/dev/null || true)"
  failed="$(rg -c 'test result: FAILED\.' "$logfile" 2>/dev/null || true)"
  printf '%s\n' "$id,$elapsed,,$,$running,$zero,$passed,$failed" >>"$OUT/metrics.csv"
  log "elapsed=${elapsed}s running_lines=${running:-0} zero_test_bins=${zero:-0}"
}

: >"$OUT/run.log"
echo "scenario,elapsed_s,user_s,sys_s,running_lines,zero_test_bins,ok_summaries,failed_summaries" >"$OUT/metrics.csv"

log "Warming build (all test targets)..."
(cd "$CRATE" && cargo test -p "$PKG" --no-run) >>"$OUT/run.log" 2>&1

log "Warming nextest metadata..."
(cd "$CRATE" && cargo nextest list -p "$PKG" --all-targets) >>"$OUT/run.log" 2>&1

run_timed cargo-filter-no-target "cargo test -p $PKG $FILTER"
run_timed cargo-filter-lib "cargo test -p $PKG --lib $FILTER"
run_timed cargo-filter-contract-module "cargo test -p $PKG --lib contract_tests"
run_timed cargo-filter-integration-one "cargo test -p $PKG --test integration_01_tests"
run_timed cargo-filter-all-lib "cargo test -p $PKG --lib"

run_timed nextest-filter-name "cargo nextest run -p $PKG -E 'test($FILTER)'"
run_timed nextest-filter-lib-only "cargo nextest run -p $PKG --lib -E 'test($FILTER)'"
run_timed nextest-filter-contract-module "cargo nextest run -p $PKG --lib -E 'test(contract_tests)'"
run_timed nextest-filter-all-lib "cargo nextest run -p $PKG --lib"
run_timed nextest-list-filter-name "cargo nextest list -p $PKG -E 'test($FILTER)'"

log ""
log "Done. Metrics: $OUT/metrics.csv"
