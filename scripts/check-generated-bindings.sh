#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

bindings_file="src/lib/generated/tauri.ts"

if [[ ! -f "$bindings_file" ]]; then
  echo "[check-generated-bindings] Missing $bindings_file. Generating it now..."
fi

echo "[check-generated-bindings] Regenerating TypeScript bindings"
bun run bindings:generate >/dev/null

if ! git diff --quiet -- "$bindings_file"; then
  echo "[check-generated-bindings] Generated bindings are stale."
  echo "[check-generated-bindings] Run: bun run bindings:generate"
  git --no-pager diff -- "$bindings_file"
  exit 1
fi

echo "[check-generated-bindings] Bindings are up to date."
