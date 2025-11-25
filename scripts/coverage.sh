#!/usr/bin/env bash
# Generate test coverage reports for Rust (cargo-tarpaulin) and TypeScript (vitest).
# Reports are output to coverage/ directory as HTML for local viewing.
#
# Usage:
#   ./scripts/coverage.sh        # Run both Rust and TypeScript coverage
#   ./scripts/coverage.sh rust   # Run only Rust coverage
#   ./scripts/coverage.sh ts     # Run only TypeScript coverage
#
# Requirements:
#   - Rust: cargo-tarpaulin (install: cargo install cargo-tarpaulin)
#   - TypeScript: vitest (install: npm install)
#
# Output:
#   - coverage/rust/tarpaulin-report.html   (Rust coverage)
#   - coverage/typescript/index.html        (TypeScript coverage)

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tauri_dir="$repo_root/src-tauri"
coverage_dir="$repo_root/coverage"

log_step() {
  echo "[coverage] $*"
}

log_warn() {
  echo "[coverage] WARNING: $*" >&2
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[coverage] Required tool '$1' not found." >&2
    return 1
  fi
}

run_rust_coverage() {
  log_step "Running Rust coverage with cargo-tarpaulin..."

  if ! require cargo; then
    log_warn "cargo not found, skipping Rust coverage"
    return 1
  fi

  if ! cargo tarpaulin --version >/dev/null 2>&1; then
    log_warn "cargo-tarpaulin not installed. Install with: cargo install cargo-tarpaulin"
    log_warn "Skipping Rust coverage"
    return 1
  fi

  mkdir -p "$coverage_dir/rust"

  (
    cd "$tauri_dir"
    # Run tarpaulin with HTML output
    # --out Html: Generate HTML report
    # --out Lcov: Generate LCOV for potential CI integration
    # --skip-clean: Don't clean before running (faster iteration)
    # --ignore-tests: Don't count test code itself in coverage
    cargo tarpaulin \
      --out Html \
      --out Lcov \
      --output-dir "$coverage_dir/rust" \
      --skip-clean \
      --ignore-tests \
      --timeout 300 \
      --exclude-files "tests/*" \
      2>&1
  )

  log_step "Rust coverage report: $coverage_dir/rust/tarpaulin-report.html"
}

run_typescript_coverage() {
  log_step "Running TypeScript coverage with vitest..."

  if ! require npm; then
    log_warn "npm not found, skipping TypeScript coverage"
    return 1
  fi

  # Check if vitest is installed
  if ! npm list vitest >/dev/null 2>&1; then
    log_warn "vitest not installed. Run 'npm install' first."
    log_warn "Skipping TypeScript coverage"
    return 1
  fi

  # Run vitest with coverage
  npm run test:coverage 2>&1

  log_step "TypeScript coverage report: $coverage_dir/typescript/index.html"
}

# Parse arguments
target="${1:-all}"

case "$target" in
  rust)
    run_rust_coverage
    ;;
  ts|typescript)
    run_typescript_coverage
    ;;
  all|"")
    rust_ok=true
    ts_ok=true

    run_rust_coverage || rust_ok=false
    run_typescript_coverage || ts_ok=false

    echo ""
    log_step "=== Coverage Summary ==="
    if $rust_ok; then
      log_step "Rust:       $coverage_dir/rust/tarpaulin-report.html"
    else
      log_step "Rust:       SKIPPED (see warnings above)"
    fi
    if $ts_ok; then
      log_step "TypeScript: $coverage_dir/typescript/index.html"
    else
      log_step "TypeScript: SKIPPED (see warnings above)"
    fi
    ;;
  *)
    echo "Usage: $0 [rust|ts|all]" >&2
    exit 1
    ;;
esac

log_step "Done."
