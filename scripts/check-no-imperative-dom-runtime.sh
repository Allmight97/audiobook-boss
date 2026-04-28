#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Scope posture:
# - Guard migrated runtime entry surfaces and boundary code.
# - Includes zero-legacy cutover lanes (status, output, file actions, metadata lookup,
#   metadata form, job controls, file import, tag preview, file-list view-state bridge).
#
# False-positive posture:
# - Exclude test/support/generated paths so jsdom fixture setup does not trip
#   this runtime-only guardrail.
runtime_scope=(
  -g 'src/App.svelte'
  -g 'src/main.ts'
  -g 'src/lib/**'
  -g 'src/ui/jobControls.ts'
  -g 'src/ui/jobControls/**'
  -g 'src/ui/statusPanel.ts'
  -g 'src/ui/statusPanel/**'
  -g 'src/ui/outputPanel.ts'
  -g 'src/ui/outputPanel/**'
  -g 'src/ui/metadataLookup.ts'
  -g 'src/ui/metadataLookup/**'
  -g 'src/ui/tagPreview.ts'
  -g 'src/ui/tagPreview/**'
  -g 'src/ui/previewAudio/**'
  -g 'src/ui/fileImport/**'
  -g 'src/ui/fileList/index.ts'
  -g 'src/ui/fileList/actions.ts'
  -g 'src/ui/fileList/dom.ts'
  -g 'src/ui/metadataForm.ts'
  -g 'src/ui/metadataForm/**'
  -g 'src/ui/core/actions.ts'
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
