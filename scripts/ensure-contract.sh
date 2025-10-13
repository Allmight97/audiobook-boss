#!/usr/bin/env bash
# NOTE: Usage review tracked in docs/planning_mapping/progress_bug_tracker.md (see "Review scripts/ensure-contract.sh").

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "== Collecting TS invoke() command names =="
ts_cmds=$(rg -n "invoke\('" "$ROOT_DIR/src" | awk -F"'" '{print $2}' | sort -u || true)
if [ -z "${ts_cmds}" ]; then
  echo "No invoke() calls found in src/"
else
  echo "$ts_cmds" | sed 's/^/  - /'
fi

echo
echo "== Collecting Rust registered command names (from generate_handler!) =="
handler_block=$(awk '/generate_handler!\[/, /\]/{print}' "$ROOT_DIR/src-tauri/src/lib.rs" || true)
rust_cmds=$(echo "$handler_block" | rg -o "[A-Za-z0-9_]+" | tail -n +2 | sort -u || true)
if [ -z "${rust_cmds}" ]; then
  echo "No commands found in generate_handler! block"
else
  echo "$rust_cmds" | sed 's/^/  - /'
fi

echo
echo "== Diff (TS minus Rust) =="
comm -23 <(echo "$ts_cmds") <(echo "$rust_cmds") || true

echo
echo "== Diff (Rust minus TS) =="
comm -13 <(echo "$ts_cmds") <(echo "$rust_cmds") || true

# Fail if any diff present
missing_in_rust=$(comm -23 <(echo "$ts_cmds") <(echo "$rust_cmds") | wc -l | tr -d ' ')
missing_in_ts=$(comm -13 <(echo "$ts_cmds") <(echo "$rust_cmds") | wc -l | tr -d ' ')

if [ "$missing_in_rust" -ne 0 ] || [ "$missing_in_ts" -ne 0 ]; then
  echo "\nContract mismatch detected. Please reconcile TS invoke() names and Rust handler registrations." 1>&2
  exit 1
else
  echo "\nContract OK: TS invoke() names match Rust handlers."
fi

