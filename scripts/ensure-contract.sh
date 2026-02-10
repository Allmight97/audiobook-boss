#!/usr/bin/env bash
# NOTE: Usage review tracked in docs/planning/progress_bug_tracker.md (see "Review scripts/ensure-contract.sh").

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Ensure required tools are present
for tool in rg awk sed comm; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Error: Required tool '$tool' not found. Please install it." >&2
    exit 1
  fi
done

log_group() {
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "::group::$1"
  else
    echo "== $1 =="
  fi
}

log_endgroup() {
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "::endgroup::"
  fi
}

log_group "Collecting TS invoke() command names"
ts_cmds=$(rg -n "invoke(?:<[^>]+>)?\('" "$ROOT_DIR/src" | awk -F"'" '{print $2}' | sort -u || true)
if [ -z "${ts_cmds}" ]; then
  echo "No invoke() calls found in src/"
else
  echo "$ts_cmds" | sed 's/^/  - /'
fi
log_endgroup

echo
log_group "Collecting Rust registered command names"
handler_block=$(awk '/generate_handler!\[/, /\]/{print}' "$ROOT_DIR/src-tauri/src/lib.rs" || true)
rust_cmds=$(echo "$handler_block" \
  | sed 's,//.*$,,' \
  | rg -o "commands::[A-Za-z0-9_]+" \
  | sed 's/commands:://' \
  | sort -u || true)

# tauri-specta path: parse collect_commands![] in ipc_contract.rs when generate_handler![] is not present
if [ -z "${rust_cmds}" ] && [ -f "$ROOT_DIR/src-tauri/src/ipc_contract.rs" ]; then
  specta_block=$(awk '/collect_commands!\[/, /\]/{print}' "$ROOT_DIR/src-tauri/src/ipc_contract.rs" || true)
  rust_cmds=$(echo "$specta_block" \
    | sed 's,//.*$,,' \
    | rg -o "crate::commands::[A-Za-z0-9_]+" \
    | sed 's/crate::commands:://' \
    | sort -u || true)
fi

if [ -z "${rust_cmds}" ]; then
  echo "No commands found in generate_handler![] or collect_commands![]"
else
  echo "$rust_cmds" | sed 's/^/  - /'
fi
log_endgroup

echo
log_group "Diff (TS minus Rust)"
comm -23 <(echo "$ts_cmds") <(echo "$rust_cmds") || true
log_endgroup

log_group "Diff (Rust minus TS)"
comm -13 <(echo "$ts_cmds") <(echo "$rust_cmds") || true
log_endgroup

# Final Reporting logic
missing_in_rust_list=$(comm -23 <(echo "$ts_cmds") <(echo "$rust_cmds"))
missing_in_ts_list=$(comm -13 <(echo "$ts_cmds") <(echo "$rust_cmds"))

missing_in_rust_count=$(echo "$missing_in_rust_list" | awk 'NF' | wc -l | tr -d ' ')
missing_in_ts_count=$(echo "$missing_in_ts_list" | awk 'NF' | wc -l | tr -d ' ')

if [ "$missing_in_rust_count" -ne 0 ]; then
  msg="Contract mismatch detected: TS invoke() names missing in Rust handler registrations"
  echo -e "\nERROR: $msg:" 1>&2
  echo "$missing_in_rust_list" | sed 's/^/  - /' 1>&2
  
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "### ❌ Contract Mismatch" >> "$GITHUB_STEP_SUMMARY"
    echo "$msg:" >> "$GITHUB_STEP_SUMMARY"
    echo "$missing_in_rust_list" | sed 's/^/ - /' >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 1
fi

if [ "$missing_in_ts_count" -ne 0 ]; then
  msg="Rust registers additional commands not yet invoked from TS"
  echo -e "\nNote: $msg:"
  echo "$missing_in_ts_list" | sed 's/^/  - /'
  
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "### ⚠️ Unused Rust Commands" >> "$GITHUB_STEP_SUMMARY"
    echo "$msg:" >> "$GITHUB_STEP_SUMMARY"
    echo "$missing_in_ts_list" | sed 's/^/ - /' >> "$GITHUB_STEP_SUMMARY"
  fi
else
  msg="Contract OK: TS invoke() names match Rust handlers"
  echo -e "\n$msg."
  if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    echo "### ✅ Contract OK" >> "$GITHUB_STEP_SUMMARY"
    echo "$msg." >> "$GITHUB_STEP_SUMMARY"
  fi
fi
