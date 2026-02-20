#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Scope posture:
# - Guard migrated runtime entry surfaces and boundary code.
# - Intentionally do not scan legacy `src/ui/**` yet; those modules are tracked
#   migration debt on the zero-legacy cutover branch.
#
# False-positive posture:
# - Exclude test/support/generated paths so jsdom fixture setup does not trip
#   this runtime-only guardrail.
runtime_scope=(
  -g 'src/App.svelte'
  -g 'src/main.ts'
  -g 'src/harness-main.ts'
  -g 'src/lib/**'
  -g 'src/ui/jobControls.ts'
  -g 'src/ui/jobControls/**'
  -g 'src/ui/metadataLookup.ts'
  -g 'src/ui/metadataLookup/**'
  -g '!src/lib/generated/**'
  -g '!src/**/*.test.*'
  -g '!src/**/__tests__/**'
  -g '!src/test/**'
)

# Banned patterns are high-signal imperative DOM mutation/query APIs that should
# not be added to migrated runtime entry paths.
pattern='document\.querySelector(All)?\(|document\.createElement\(|\.innerHTML\s*=|\.outerHTML\s*=|insertAdjacentHTML\(|document\.write\('

if rg -n --pcre2 "$pattern" src "${runtime_scope[@]}" >/tmp/no-imperative-dom-runtime.out 2>/dev/null; then
  echo "[no-imperative-dom-runtime] Found banned imperative DOM pattern(s) in guarded runtime paths." >&2
  cat /tmp/no-imperative-dom-runtime.out >&2
  rm -f /tmp/no-imperative-dom-runtime.out
  exit 1
fi

rm -f /tmp/no-imperative-dom-runtime.out
echo "[no-imperative-dom-runtime] OK"
