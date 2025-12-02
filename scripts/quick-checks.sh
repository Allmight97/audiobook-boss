#!/usr/bin/env bash
# Runs the fast "green baseline" checks documented in AGENTS.md.
# Exits on the first failure so callers get clear feedback.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tauri_dir="$repo_root/src-tauri"

log_step() {
  echo "[quick-checks] $*"
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[quick-checks] Required tool '$1' not found." >&2
    exit 1
  fi
}

log_step "cargo fmt --all -- --check"
require cargo
if [[ ! -d "$tauri_dir" ]]; then
  echo "[quick-checks] Expected directory '$tauri_dir' not found." >&2
  exit 1
fi
(
  cd "$tauri_dir"
  cargo fmt --all -- --check
)

log_step "cargo clippy --workspace --all-targets -- -D warnings"
(
  cd "$tauri_dir"
  cargo clippy --workspace --all-targets -- -D warnings
)

if [[ -x scripts/ensure-contract.sh ]]; then
  log_step "scripts/ensure-contract.sh"
  scripts/ensure-contract.sh
elif [[ -f scripts/ensure-contract.sh ]]; then
  log_step "bash scripts/ensure-contract.sh (non-executable file)"
  bash scripts/ensure-contract.sh
else
  echo "[quick-checks] scripts/ensure-contract.sh not executable or missing." >&2
  exit 1
fi

if [[ "${SKIP_TS_CHECK:-0}" == "1" ]]; then
  log_step "Skipping TypeScript typecheck (SKIP_TS_CHECK=1)."
else
  if command -v bunx >/dev/null 2>&1; then
    log_step "bunx tsc -p tsconfig.json --noEmit"
    bunx tsc -p tsconfig.json --noEmit
  else
    log_step "Skipping TypeScript typecheck (bunx not found)."
  fi
fi

log_step "All quick checks passed."
