#!/usr/bin/env bash
# Primary repo quality gate for humans + agents.
#
# Usage:
#   scripts/checks.sh [quick|standard|release]
#   scripts/checks.sh --tier quick|standard|release
#
# Defaults to "standard".
#
# Tiers:
# - quick: Rust fmt + frontend format + lint + clippy + IPC binding drift + fallback policy enforcement
# - standard: quick + Rust tests + TS tests + app build
# - release: standard + release build
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
  scripts/checks.sh [quick|standard|release]
  scripts/checks.sh --tier quick|standard|release
USAGE
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    quick|standard|release)
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
  quick|standard|release) ;;
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
  # support remains a release-risky surface for this repo. issue=#219
  # sunset=2026-06-30
  log_step "bun run fmt:check"
  bun run fmt:check

  log_step "cargo clippy --workspace --all-targets -- -D warnings"
  cargo clippy --workspace --all-targets -- -D warnings

  log_step "scripts/check-generated-bindings.sh"
  bash scripts/check-generated-bindings.sh

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
}

run_release() {
  run_standard

  log_step "cargo build --release -p audiobook-boss"
  cargo build --release -p audiobook-boss
}

case "$tier" in
  quick) run_quick ;;
  standard) run_standard ;;
  release) run_release ;;
esac

log_step "All checks passed."
