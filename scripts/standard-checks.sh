#!/usr/bin/env bash
# Runs the Standard tier checks documented in AGENTS.md.
# Standard = Quick (without TS check) + cargo test + bun run build.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

log_step() {
  echo "[standard-checks] $*"
}

log_step "scripts/quick-checks.sh (SKIP_TS_CHECK=1)"
SKIP_TS_CHECK=1 scripts/quick-checks.sh

log_step "cargo test"
cargo test

log_step "bun run build"
bun run build

log_step "All standard checks passed."
