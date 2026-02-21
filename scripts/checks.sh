#!/usr/bin/env bash
# Primary repo quality gate for humans + agents.
#
# Usage:
#   scripts/checks.sh [quick|standard|package]
#   scripts/checks.sh --tier quick|standard|package
#
# Defaults to "standard".
#
# Tiers:
# - quick: Rust fmt + frontend format + lint + clippy + change-aware IPC binding drift check + runtime guardrails + fallback policy enforcement
# - standard: quick + Rust tests + TS tests + app build
# - package: standard + Tauri app bundling (validates real packaging path)
#
# The intent is behavior-first confidence:
# if `scripts/checks.sh standard` is green, the branch is safe
# to send for PR review.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tier="standard"

usage() {
  cat <<'USAGE'
Usage:
  scripts/checks.sh [quick|standard|package]
  scripts/checks.sh --tier quick|standard|package
USAGE
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    quick|standard|package)
      tier="$1"
      shift
      ;;
    --tier)
      if [[ $# -lt 2 ]]; then
        usage
        exit 1
      fi
      tier="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[checks] Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
fi

if [[ $# -gt 0 ]]; then
  echo "[checks] Unexpected arguments: $*" >&2
  usage
  exit 1
fi

case "$tier" in
  quick|standard|package) ;;
  *)
    echo "[checks] Invalid tier: $tier" >&2
    usage
    exit 1
    ;;
esac

log_step() {
  echo "[checks:$tier] $*"
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[checks:$tier] Required tool '$1' not found." >&2
    exit 1
  fi
}

run_quick() {
  require cargo
  require bun

  log_step "cargo fmt --all -- --check"
  cargo fmt --all -- --check

  # FALLBACK[FB-018]: Keep Prettier checks for .svelte while Biome Svelte formatting
  # support remains a migration-risky surface for this repo. issue=#219
  # sunset=2026-06-30
  log_step "bun run fmt:check"
  bun run fmt:check

  log_step "cargo clippy --workspace --all-targets -- -D warnings"
  cargo clippy --workspace --all-targets -- -D warnings

  if [[ "${CHECK_BINDINGS_STRICT:-0}" == "1" ]]; then
    log_step "scripts/check-generated-bindings.sh --mode verify (strict)"
    bash scripts/check-generated-bindings.sh --mode verify
  else
    log_step "scripts/check-generated-bindings.sh --mode local"
    bash scripts/check-generated-bindings.sh --mode local
  fi

  log_step "scripts/check-no-bridge-imports.sh"
  bash scripts/check-no-bridge-imports.sh

  log_step "scripts/check-no-imperative-dom-runtime.sh"
  bash scripts/check-no-imperative-dom-runtime.sh

  log_step "scripts/check-no-legacy-test-contracts.sh"
  bash scripts/check-no-legacy-test-contracts.sh

  log_step "scripts/check-fallback-policy.sh"
  bash scripts/check-fallback-policy.sh
}

run_standard() {
  run_quick

  log_step "cargo test"
  cargo test

  log_step "bun run test"
  bun run test

  log_step "bun run build"
  bun run build

  require cargo-audit
  log_step "cargo audit -D warnings"
  cargo audit -D warnings
}

run_package() {
  run_standard

  log_step "bun run app:build (Tauri app packaging)"
  bun run app:build
}

case "$tier" in
  quick) run_quick ;;
  standard) run_standard ;;
  package) run_package ;;
esac

log_step "All checks passed."
