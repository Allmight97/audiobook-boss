#!/usr/bin/env bash
# Fails when a Rust function passes cyclomatic complexity 20 and is not listed in
# scripts/rust-complexity-allowlist.txt. Test code is skipped. Lizard parses
# without compiling. The length limit is raised because lizard can misread where
# a function ends (a 13-complexity function was reported 1000+ lines long).
# Lizard is pinned so the same code receives the same complexity verdict.
# Update through the 10-day release-age policy (scripts/AGENTS.md).
# Usage: bash scripts/check-rust-complexity.sh [--prefetch]
set -euo pipefail
cd "$(dirname "$0")/.."

lizard=(uvx --exclude-newer "10 days" --from "lizard==1.24.1" lizard)
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
