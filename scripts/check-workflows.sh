#!/usr/bin/env bash
# Lints GitHub workflows: actionlint (syntax, expressions, and run: blocks
# through shellcheck) and zizmor (security). zizmor runs through uvx with the
# pinned version and 10-day wait (scripts/AGENTS.md); --offline skips
# audits that query GitHub, so the result does not change between runs.
# Usage: bash scripts/check-workflows.sh [--prefetch]
set -euo pipefail
cd "$(dirname "$0")/.."

zizmor=(uvx --exclude-newer "10 days" --from "zizmor==1.30.1" zizmor)
if [[ "${1:-}" == --version ]]; then
 "${zizmor[@]}" --version
 exit 0
fi
if [[ "${1:-}" == --prefetch ]]; then
	"${zizmor[@]}" --version >/dev/null
	exit 0
fi

actionlint
"${zizmor[@]}" --offline .github/workflows
