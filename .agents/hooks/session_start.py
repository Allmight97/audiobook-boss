from __future__ import annotations

from common import emit


emit(
    {
        "continue": True,
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": (
                "ABB hooks active: docs-only edits use "
                "`bash scripts/check-context-surface.sh`; non-doc code changes use "
                "`scripts/checks.sh standard`; IPC boundary edits should keep "
                "`src/lib/generated/tauri.ts` in sync."
            ),
        },
    }
)
