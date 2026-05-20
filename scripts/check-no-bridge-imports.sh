#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ -f "src/lib/bridge.ts" ]]; then
  echo "[no-bridge] src/lib/bridge.ts must not exist." >&2
  exit 1
fi

# Reject any relative import/reference depth (./, ../, ../../../, etc.) and
# optional explicit extensions (e.g. lib/bridge.ts) under src/.
if rg -n "['\"](?:\\./|\\.\\./)*lib/bridge(?:\\.[a-zA-Z0-9]+)?['\"]" src >/dev/null 2>&1; then
  echo "[no-bridge] Found disallowed bridge import/reference under src/." >&2
  rg -n "['\"](?:\\./|\\.\\./)*lib/bridge(?:\\.[a-zA-Z0-9]+)?['\"]" src
  exit 1
fi

if ! bun scripts/check-generated-tauri-imports.ts >/tmp/no-generated-tauri-imports.out 2>&1; then
  echo "[no-bridge] Generated command/event value imports must stay inside the Tauri runtime boundary." >&2
  cat /tmp/no-generated-tauri-imports.out >&2
  rm -f /tmp/no-generated-tauri-imports.out
  exit 1
fi
rm -f /tmp/no-generated-tauri-imports.out

if [[ -f "src/ui/outputPanel/pathBuilder.ts" ]]; then
  echo "[no-bridge] Frontend output path naming mirrors must not exist; use tauriClient.previewOutputPath." >&2
  exit 1
fi

if rg -n "['\"][^'\"]*outputPanel/pathBuilder(?:\\.[a-zA-Z0-9]+)?['\"]" src \
  -g '*.ts' \
  -g '*.svelte' \
  -g '!src/lib/generated/tauri.ts' \
  >/tmp/no-output-path-builder-imports.out 2>/dev/null; then
  echo "[no-bridge] Frontend code must not import an output path naming mirror." >&2
  cat /tmp/no-output-path-builder-imports.out >&2
  rm -f /tmp/no-output-path-builder-imports.out
  exit 1
fi
rm -f /tmp/no-output-path-builder-imports.out

if rg -n "\\b(export\\s+)?function\\s+(calculateOutputPath|sanitizeFilename)\\b|\\b(calculateOutputPath|sanitizeFilename)\\s*[:=(]" src/ui \
  -g '*.ts' \
  -g '*.svelte' \
  >/tmp/no-output-path-builder-symbols.out 2>/dev/null; then
  echo "[no-bridge] Output path naming must stay in the Rust output_artifact boundary." >&2
  cat /tmp/no-output-path-builder-symbols.out >&2
  rm -f /tmp/no-output-path-builder-symbols.out
  exit 1
fi
rm -f /tmp/no-output-path-builder-symbols.out

if rg -n "@tauri-apps/api/core|__TAURI_INVOKE|\\binvoke\\(" src \
  -g '*.ts' \
  -g '*.svelte' \
  -g '!src/lib/generated/tauri.ts' \
  -g '!src/**/*.test.ts' \
  -g '!src/**/__tests__/**' \
  -g '!src/test/**' \
  >/tmp/no-direct-tauri-core.out 2>/dev/null; then
  echo "[no-bridge] Runtime code must not call the raw Tauri core invoke API." >&2
  cat /tmp/no-direct-tauri-core.out >&2
  rm -f /tmp/no-direct-tauri-core.out
  exit 1
fi
rm -f /tmp/no-direct-tauri-core.out

if rg -n "\\b(commit_output_artifact|finalized_output_success|OutputPlanLedger)\\b" src-tauri/src \
  -g '*.rs' \
  -g '!src-tauri/src/output_artifact/**' \
  -g '!src-tauri/src/audio/processor/finalize.rs' \
  -g '!src-tauri/src/processing/plan.rs' \
  >/tmp/no-output-path-reach-through.out 2>/dev/null; then
  echo "[no-bridge] Output artifact truth may only be used by allowlisted boundary consumers." >&2
  cat /tmp/no-output-path-reach-through.out >&2
  rm -f /tmp/no-output-path-reach-through.out
  exit 1
fi
rm -f /tmp/no-output-path-reach-through.out

if rg -n "\\b(resolve_effective_processing_metadata|resolve_naming_metadata)\\b" src-tauri/src \
  -g '*.rs' \
  -g '!src-tauri/src/metadata/**' \
  -g '!src-tauri/src/processing/plan.rs' \
  >/tmp/no-metadata-intent-reach-through.out 2>/dev/null; then
  echo "[no-bridge] Metadata intent projection may only be used by the processing planner boundary." >&2
  cat /tmp/no-metadata-intent-reach-through.out >&2
  rm -f /tmp/no-metadata-intent-reach-through.out
  exit 1
fi
rm -f /tmp/no-metadata-intent-reach-through.out

if rg -n "['\"][^'\"]*statusPanel/(viewState|controller|runtimeApi|domain/stateMachine|render|feedback|reducer|processingWorkflow|processingCancellationWorkflow)" src \
  -g '*.ts' \
  -g '*.svelte' \
  -g '!src/ui/statusPanel/**' \
  >/tmp/no-status-panel-private-imports.out 2>/dev/null; then
  echo "[no-bridge] Code outside src/ui/statusPanel must use the status panel public API." >&2
  cat /tmp/no-status-panel-private-imports.out >&2
  rm -f /tmp/no-status-panel-private-imports.out
  exit 1
fi
rm -f /tmp/no-status-panel-private-imports.out

if rg -n "std::fs::(rename|copy|hard_link)" src-tauri/src/audio/processor/finalize.rs \
  >/tmp/no-finalize-artifact-commit.out 2>/dev/null; then
  echo "[no-bridge] Processor finalization must not perform final artifact file commits directly." >&2
  cat /tmp/no-finalize-artifact-commit.out >&2
  rm -f /tmp/no-finalize-artifact-commit.out
  exit 1
fi
rm -f /tmp/no-finalize-artifact-commit.out

echo "[no-bridge] OK"
