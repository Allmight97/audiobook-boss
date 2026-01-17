#!/usr/bin/env bash
# Runs the Release tier checks documented in AGENTS.md.
# Release = Standard + cargo build --release -p audiobook-boss.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

log_step() {
  echo "[release-checks] $*"
}

log_step "scripts/standard-checks.sh"
scripts/standard-checks.sh

log_step "cargo build --release -p audiobook-boss"
cargo build --release -p audiobook-boss

log_step "All release checks passed."
