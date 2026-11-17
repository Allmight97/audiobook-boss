#!/bin/bash
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
	exit 0
fi

repo_root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
bash "${repo_root}/scripts/setup.sh" frontend >&2

if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
	eval "$("${repo_root}/scripts/setup.sh" --print-env | tee -a "${CLAUDE_ENV_FILE}")"
fi
