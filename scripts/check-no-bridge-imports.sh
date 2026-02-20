#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ -f "src/lib/bridge.ts" ]]; then
  echo "[no-bridge] src/lib/bridge.ts must not exist." >&2
  exit 1
fi

if rg -n "['\"](\\./|\\.\\./|\\.\\./\\.\\./)?lib/bridge(['\"]|$)" src >/dev/null 2>&1; then
  echo "[no-bridge] Found disallowed bridge import/reference under src/." >&2
  rg -n "['\"](\\./|\\.\\./|\\.\\./\\.\\./)?lib/bridge(['\"]|$)" src
  exit 1
fi

echo "[no-bridge] OK"
