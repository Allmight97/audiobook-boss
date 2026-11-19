#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

lizard=(uvx --exclude-newer "10 days" lizard)
if [[ "${1:-}" == --version ]]; then
	"${lizard[@]}" --version
	exit 0
fi
if [[ "${1:-}" == --prefetch ]]; then
	"${lizard[@]}" --version >/dev/null
	exit 0
fi

"${lizard[@]}" crates/*/src src-tauri/src \
	--CCN 20 --length 100000 --warnings_only \
	--whitelist scripts/rust-complexity-allowlist.txt \
	--exclude "*_tests.rs" --exclude "*/tests.rs" --exclude "*/test_cases/*"
