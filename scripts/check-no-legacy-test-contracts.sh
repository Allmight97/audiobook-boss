#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

test_scope=(
  -g 'src/**/*.test.*'
  -g 'src/**/__tests__/**'
  -g 'src/test/**'
)

pattern='EncoderSettingsProvider|userStatusLockUntil|abb:encoder-settings-changed|window\.currentCoverArt'

if rg -n --pcre2 "$pattern" src "${test_scope[@]}" >/tmp/no-legacy-test-contracts.out 2>/dev/null; then
  echo "[no-legacy-test-contracts] Found disallowed legacy contract reference(s) in tests." >&2
  cat /tmp/no-legacy-test-contracts.out >&2
  rm -f /tmp/no-legacy-test-contracts.out
  exit 1
fi

rm -f /tmp/no-legacy-test-contracts.out
echo "[no-legacy-test-contracts] OK"
