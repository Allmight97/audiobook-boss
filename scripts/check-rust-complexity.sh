#!/usr/bin/env bash
# Fails when a Rust function passes cyclomatic complexity 20 and is not listed in
# scripts/rust-complexity-allowlist.txt. Test code is skipped. Lizard parses
# without compiling. The length limit is raised because lizard can misread where
# a function ends (a 13-complexity function was reported 1000+ lines long).
# Usage: bash scripts/check-rust-complexity.sh
set -euo pipefail
cd "$(dirname "$0")/.."

uvx lizard==1.24.0 crates/*/src src-tauri/src \
	--CCN 20 --length 100000 --warnings_only \
	--whitelist scripts/rust-complexity-allowlist.txt \
	--exclude "*_tests.rs" --exclude "*/tests.rs" --exclude "*/test_cases/*"
