#!/bin/bash
set -euo pipefail

# Claude Code cloud sessions start without node_modules and may carry a Bun
# other than package.json#packageManager. Pin Bun and install frontend deps so
# typecheck, Biome, and Vitest run. The FFmpeg/Tauri media lane stays with the
# environment's setup script (scripts/AGENTS.md, Linux Agent Environment).
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
	exit 0
fi

repo_root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
# Hook stdout becomes session context; send setup logs to stderr.
bash "${repo_root}/scripts/setup-codex-agent-env.sh" --frontend-only >&2

if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
	echo 'export PATH="$HOME/.bun/bin:$PATH"' >> "${CLAUDE_ENV_FILE}"
fi
