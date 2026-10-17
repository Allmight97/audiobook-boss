#!/usr/bin/env bash
# Verify generated bindings without changing the candidate being proved.
# Usage: bash scripts/check-generated-bindings.sh
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"
eval "$(scripts/setup.sh --print-env)"
if [[ $# -ne 0 ]]; then
 printf 'Usage: bash scripts/check-generated-bindings.sh\n' >&2
 exit 2
fi
bindings_file="src/lib/generated/tauri.ts"
temporary="$(mktemp -d)"
trap 'rm -rf "${temporary}"' EXIT
bun run bindings:generate "${temporary}/tauri.ts"
if [[ ! -f "${bindings_file}" ]]; then
 printf 'Missing bindings: run bun run bindings:generate\n' >&2
 exit 1
fi
if ! cmp -s "${bindings_file}" "${temporary}/tauri.ts"; then
 printf 'Generated bindings are stale: run bun run bindings:generate\n' >&2
 diff -u "${bindings_file}" "${temporary}/tauri.ts" || [[ $? -eq 1 ]]
 exit 1
fi
printf 'Bindings are up to date.\n'
