#!/usr/bin/env bash
# Runs the Quick tier checks documented in AGENTS.md.
# Exits on the first failure so callers get clear feedback.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tauri_manifest="$repo_root/src-tauri/Cargo.toml"
workspace_manifest="$repo_root/Cargo.toml"

log_step() {
  echo "[quick-checks] $*"
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[quick-checks] Required tool '$1' not found." >&2
    exit 1
  fi
}

cargo_fmt_cmd() {
  if [[ -f "$workspace_manifest" ]]; then
    cargo fmt --all -- --check
    return
  fi

  if [[ -f "$tauri_manifest" ]]; then
    cargo fmt --manifest-path "$tauri_manifest" --all -- --check
    return
  fi

  echo "[quick-checks] No Cargo manifest found." >&2
  exit 1
}

cargo_clippy_cmd() {
  if [[ -f "$workspace_manifest" ]]; then
    cargo clippy --workspace --all-targets -- -D warnings
    return
  fi

  if [[ -f "$tauri_manifest" ]]; then
    cargo clippy --manifest-path "$tauri_manifest" --all-targets -- -D warnings
    return
  fi

  echo "[quick-checks] No Cargo manifest found." >&2
  exit 1
}

log_step "cargo fmt --all -- --check"
require cargo
cargo_fmt_cmd

log_step "cargo clippy --workspace --all-targets -- -D warnings"
cargo_clippy_cmd

if [[ -x scripts/check-generated-bindings.sh ]]; then
  log_step "scripts/check-generated-bindings.sh"
  scripts/check-generated-bindings.sh
elif [[ -f scripts/check-generated-bindings.sh ]]; then
  log_step "bash scripts/check-generated-bindings.sh (non-executable file)"
  bash scripts/check-generated-bindings.sh
else
  echo "[quick-checks] scripts/check-generated-bindings.sh not executable or missing." >&2
  exit 1
fi

if [[ -x scripts/ensure-contract.sh ]]; then
  log_step "scripts/ensure-contract.sh (advisory)"
  scripts/ensure-contract.sh || echo "[quick-checks] Advisory: ensure-contract reported differences."
elif [[ -f scripts/ensure-contract.sh ]]; then
  log_step "bash scripts/ensure-contract.sh (advisory)"
  bash scripts/ensure-contract.sh || echo "[quick-checks] Advisory: ensure-contract reported differences."
else
  log_step "Skipping ensure-contract advisory check (script missing)."
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
