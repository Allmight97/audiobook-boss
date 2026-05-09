from __future__ import annotations

from common import emit


emit(
    {
        "continue": True,
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": (
                "ABB hooks active: PreToolUse blocks destructive Bash worktree commands; "
                "UserPromptSubmit adds advisory skill-routing hints; Stop consolidates "
                "context-surface, IPC binding, fallback-policy, verification-lane, and "
                "UI-review reminders. Docs-only edits use `bash scripts/check-context-surface.sh`; "
                "non-doc code changes use `scripts/checks.sh standard`."
            ),
        },
    }
)
