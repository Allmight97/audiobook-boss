#!/usr/bin/env bash
# Canonical proof router for Audiobook Boss humans and agents.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

route="standard"

usage() {
  cat <<'USAGE'
Usage:
  scripts/proof.sh [route] [route-args...]
  scripts/proof.sh --route <route> [route-args...]
  scripts/proof.sh --help

Routes:
  quick
      Static proof: Rust fmt, frontend format/lint, clippy, generated binding
      drift, boundary assertions, and fallback policy.

  standard
      Final review proof: quick + Rust proof + script tests + frontend tests +
      production frontend build.

  package
      Release/package proof: standard + Tauri app bundle + AAC decoder
      contract binary.

  rust
      Full non-ignored Rust proof through Cargo/libtest.

  rust-private
      Rust private-cluster proof for source-tree unit tests and internals.

  rust-contract
      Rust public-strip/contract proof plus public API strip assertions.

  rust-media
      Committed-fixture Rust media proof. Uses repo fixtures only.

  rust-media-manual [all|xhe-aac|native-fastpath]
      Explicit manual/deep media proof. xHE-AAC requires ABB_XHE_AAC_FIXTURE
      and optionally ABB_XHE_AAC_FFMPEG. Defaults to all.

  frontend
      TypeScript/Svelte proof through Vitest.

  runtime
      Tauri runtime boundary proof: generated binding drift + runtime adapter
      contract tests.

  coverage [rust|ts|all]
      Explicit coverage diagnostic. Not a release blocker by default.

  timing [cargo build args...]
      Build feedback diagnostic. Defaults to cargo build --timings.

  deps
      Dependency hygiene proof.

Examples:
  scripts/proof.sh quick
  scripts/proof.sh standard
  scripts/proof.sh rust-contract
  scripts/proof.sh rust-media-manual xhe-aac
  ABB_XHE_AAC_FIXTURE=/path/to/book.m4b scripts/proof.sh rust-media-manual
USAGE
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --route)
      if [[ $# -lt 2 ]]; then
        echo "[proof] Missing route after --route." >&2
        usage >&2
        exit 1
      fi
      route="$2"
      shift 2
      ;;
    *)
      route="$1"
      shift
      ;;
  esac
fi

log_step() {
  echo "[proof:$route] $*"
}

fail() {
  echo "[proof:$route] $*" >&2
  exit 1
}

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Required tool '$1' not found."
  fi
}

reject_args() {
  if [[ $# -gt 0 ]]; then
    fail "Route '$route' does not accept arguments: $*"
  fi
}

run_quick() {
  reject_args "$@"
  require cargo
  require bun

  log_step "cargo fmt --all -- --check"
  cargo fmt --all -- --check

  # FALLBACK[FB-018]: Keep Prettier checks for .svelte while Biome Svelte formatting
  # support remains a migration-risky surface for this repo. issue=#219
  # sunset=2026-06-30
  log_step "bun run fmt:check"
  bun run fmt:check

  log_step "bun run lint:check"
  bun run lint:check

  log_step "cargo clippy --workspace --all-targets -- -D warnings"
  cargo clippy --workspace --all-targets -- -D warnings

  if [[ "${CHECK_BINDINGS_STRICT:-0}" == "1" ]]; then
    log_step "scripts/check-generated-bindings.sh --mode verify (strict)"
    bash scripts/check-generated-bindings.sh --mode verify
  else
    log_step "scripts/check-generated-bindings.sh --mode local"
    bash scripts/check-generated-bindings.sh --mode local
  fi

  log_step "scripts/check-public-api-strips.sh"
  bash scripts/check-public-api-strips.sh

  log_step "scripts/check-no-bridge-imports.sh"
  bash scripts/check-no-bridge-imports.sh

  log_step "scripts/check-no-imperative-dom-runtime.sh"
  bash scripts/check-no-imperative-dom-runtime.sh

  log_step "scripts/check-no-legacy-test-contracts.sh"
  bash scripts/check-no-legacy-test-contracts.sh

  log_step "scripts/check-fallback-policy.sh"
  bash scripts/check-fallback-policy.sh
}

run_rust() {
  reject_args "$@"
  require cargo
  log_step "cargo test"
  cargo test
}

run_rust_private() {
  reject_args "$@"
  require cargo
  log_step "cargo test -p audiobook-boss --lib"
  cargo test -p audiobook-boss --lib
}

run_rust_contract() {
  reject_args "$@"
  require cargo
  log_step "cargo test -p audiobook-boss contract_tests"
  cargo test -p audiobook-boss contract_tests

  log_step "scripts/check-public-api-strips.sh"
  bash scripts/check-public-api-strips.sh
}

run_rust_media() {
  reject_args "$@"
  require cargo
  log_step "cargo test -p audiobook-boss --test integration_file_list_tests"
  cargo test -p audiobook-boss --test integration_file_list_tests

  log_step "cargo test -p audiobook-boss --test integration_metadata_tests"
  cargo test -p audiobook-boss --test integration_metadata_tests

  log_step "cargo test -p audiobook-boss --test integration_metadata_reader_matrix_tests"
  cargo test -p audiobook-boss --test integration_metadata_reader_matrix_tests

  log_step "cargo test -p audiobook-boss --test integration_native_aac_regression_tests"
  cargo test -p audiobook-boss --test integration_native_aac_regression_tests

  log_step "cargo test -p audiobook-boss --test integration_processing_flow_tests"
  cargo test -p audiobook-boss --test integration_processing_flow_tests
}

run_manual_xhe_aac() {
  require cargo
  if [[ -z "${ABB_XHE_AAC_FIXTURE:-}" ]]; then
    fail "ABB_XHE_AAC_FIXTURE must point to a local xHE-AAC/USAC audiobook fixture. Optionally set ABB_XHE_AAC_FFMPEG to a specific FDK-capable FFmpeg."
  fi

  log_step "cargo test -p audiobook-boss --test integration_xhe_aac_fixture_tests -- --ignored"
  cargo test -p audiobook-boss --test integration_xhe_aac_fixture_tests -- --ignored
}

run_manual_native_fastpath() {
  require cargo
  log_step "cargo test -p audiobook-boss --test integration_fastpath_parity_tests -- --ignored"
  cargo test -p audiobook-boss --test integration_fastpath_parity_tests -- --ignored
}

run_rust_media_manual() {
  if [[ $# -gt 1 ]]; then
    fail "Route '$route' accepts at most one target: all, xhe-aac, or native-fastpath."
  fi
  local target="${1:-all}"
  case "$target" in
    all)
      run_manual_xhe_aac
      run_manual_native_fastpath
      ;;
    xhe-aac)
      run_manual_xhe_aac
      ;;
    native-fastpath)
      run_manual_native_fastpath
      ;;
    *)
      fail "Unknown rust-media-manual target '$target'. Use all, xhe-aac, or native-fastpath."
      ;;
  esac
}

run_script_tests() {
  reject_args "$@"
  require bun
  log_step "bun test scripts/*.test.ts proof subset"
  bun test \
    scripts/build-app.test.ts \
    scripts/check-fallback-policy.test.ts \
    scripts/check-no-bridge-imports.test.ts \
    scripts/resolve-release-dmg.test.ts
}

run_frontend() {
  reject_args "$@"
  require bun
  log_step "bun run test"
  bun run test
}

run_runtime() {
  reject_args "$@"
  require bun
  log_step "scripts/check-generated-bindings.sh --mode local"
  bash scripts/check-generated-bindings.sh --mode local

  log_step "runtime boundary Vitest contract tests"
  bun run test -- \
    src/lib/behavior-contract.test.ts \
    src/lib/tauri-client.test.ts \
    src/lib/tauri-client.generated-event-bindings.test.ts \
    src/lib/tauri-public-api.contract.test.ts
}

run_standard() {
  reject_args "$@"
  run_quick
  run_rust
  run_script_tests
  run_frontend

  log_step "bun run build"
  bun run build
}

run_package() {
  reject_args "$@"
  run_standard

  log_step "bun run app:build (Tauri app packaging)"
  bun run app:build

  log_step "cargo run --manifest-path src-tauri/Cargo.toml --bin verify_aac_decoder_contract"
  cargo run --manifest-path src-tauri/Cargo.toml --bin verify_aac_decoder_contract --quiet
}

run_coverage() {
  local target="${1:-all}"
  log_step "scripts/coverage.sh $target"
  bash scripts/coverage.sh "$target"
}

run_timing() {
  require cargo
  if [[ $# -eq 0 ]]; then
    log_step "cargo build --timings"
    cargo build --timings
  else
    log_step "cargo build --timings $*"
    cargo build --timings "$@"
  fi
}

run_deps() {
  require bun
  log_step "bun run check:deps"
  bun run check:deps
}

case "$route" in
  quick) run_quick "$@" ;;
  standard) run_standard "$@" ;;
  package) run_package "$@" ;;
  rust) run_rust "$@" ;;
  rust-private) run_rust_private "$@" ;;
  rust-contract) run_rust_contract "$@" ;;
  rust-media) run_rust_media "$@" ;;
  rust-media-manual) run_rust_media_manual "$@" ;;
  frontend) run_frontend "$@" ;;
  runtime) run_runtime "$@" ;;
  coverage) run_coverage "$@" ;;
  timing) run_timing "$@" ;;
  deps) run_deps "$@" ;;
  *)
    echo "[proof] Unknown route: $route" >&2
    usage >&2
    exit 1
    ;;
esac

log_step "Proof route passed."
