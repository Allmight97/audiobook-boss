#!/usr/bin/env bash
# Lints GitHub workflows: actionlint (syntax, expressions, and run: blocks
# through shellcheck) and zizmor (security). zizmor runs through uvx with the
# 10-day release-age wait (scripts/AGENTS.md, Dependencies); --offline skips
# audits that query GitHub, so the result does not change between runs.
# Usage: bash scripts/check-workflows.sh [--prefetch]
set -euo pipefail
cd "$(dirname "$0")/.."

zizmor=(uvx --exclude-newer "10 days" zizmor)
if [[ "${1:-}" == --prefetch ]]; then
	"${zizmor[@]}" --version >/dev/null
	exit 0
fi

actionlint
"${zizmor[@]}" --offline .github/workflows
