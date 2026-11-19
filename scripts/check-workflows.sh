#!/usr/bin/env bash
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
