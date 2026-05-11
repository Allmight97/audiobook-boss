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

if rg -n "commands as generatedCommands" src \
  -g '*.ts' \
  -g '*.svelte' \
  -g '!src/lib/tauri/commands.ts' \
  -g '!src/lib/generated/tauri.ts' \
  >/tmp/no-generated-command-imports.out 2>/dev/null; then
  echo "[no-bridge] Generated command invokers must stay inside src/lib/tauri/commands.ts." >&2
  cat /tmp/no-generated-command-imports.out >&2
  rm -f /tmp/no-generated-command-imports.out
  exit 1
fi
rm -f /tmp/no-generated-command-imports.out

if rg -n "events as generatedEvents" src \
  -g '*.ts' \
  -g '*.svelte' \
  -g '!src/lib/tauri/client.ts' \
  -g '!src/lib/generated/tauri.ts' \
  >/tmp/no-generated-event-imports.out 2>/dev/null; then
  echo "[no-bridge] Generated event listeners must stay inside src/lib/tauri/client.ts." >&2
  cat /tmp/no-generated-event-imports.out >&2
  rm -f /tmp/no-generated-event-imports.out
  exit 1
fi
rm -f /tmp/no-generated-event-imports.out

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

echo "[no-bridge] OK"
