#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"

route="${1:-proof}"
bash scripts/greenproof/init-proof-dir.sh
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
start_ms="$(python3 -c 'import time; print(int(time.time()*1000))')"
status=0

run_step() {
  local id="$1"
  shift
  if ! bash scripts/greenproof/run-with-evidence.sh "$id" "$@"; then
    status=1
    return 1
  fi
}

case "$route" in
  proof)
    run_step proof:quick "mise run proof:quick:steps" || true
    run_step test:scripts "bun test scripts/build-app.test.ts scripts/check-fallback-policy.test.ts scripts/check-no-bridge-imports.test.ts scripts/greenproof/mise-rust-tasks.test.ts scripts/resolve-release-dmg.test.ts" || true
    run_step test:rust "bash -c 'mkdir -p .proof/latest/reports && cargo nextest run --workspace --profile ci && cp target/nextest/ci/nextest-junit.xml .proof/latest/reports/nextest-junit.xml'" || true
    run_step test:frontend "bun run test" || true
    run_step frontend-build "bun run build" || true
    ;;
  proof:release)
    bash "$0" proof || status=1
    run_step app-build "bun run app:build" || true
    run_step aac-decoder-contract "cargo run --manifest-path src-tauri/Cargo.toml --bin verify_aac_decoder_contract --quiet" || true
    route="proof:release"
    ;;
  rust:contract)
    run_step rust:contract-lib "cargo test -p audiobook-boss --lib contract_tests" || true
    run_step public-api-strips "bash scripts/check-public-api-strips.sh" || true
    ;;
  *)
    echo "unknown evidence route: $route" >&2
    exit 2
    ;;
esac

end_ms="$(python3 -c 'import time; print(int(time.time()*1000))')"
duration_ms=$((end_ms - start_ms))
bash scripts/greenproof/finalize-summary.sh "$route" "$started_at" "$duration_ms"
exit "$status"
