#!/bin/bash
set -euo pipefail

# Claude Code cloud sessions start without node_modules and may carry a Bun
# other than the one bun.lock resolves.
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
	exit 0
fi

repo_root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
# Hook stdout becomes session context; send setup logs to stderr.
bash "${repo_root}/scripts/setup.sh" frontend >&2

if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
	eval "$("${repo_root}/scripts/setup.sh" --print-env | tee -a "${CLAUDE_ENV_FILE}")"
fi
