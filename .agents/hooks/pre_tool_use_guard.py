from __future__ import annotations

import re

from common import emit, read_hook_input


DESTRUCTIVE_RULES: tuple[tuple[re.Pattern[str], str], ...] = (
    (
        re.compile(r"\bgit\s+reset\s+--hard\b"),
        "`git reset --hard` can destroy user worktree changes. Ask explicitly before using it.",
    ),
    (
        re.compile(r"\bgit\s+checkout\s+--\s+"),
        "`git checkout -- <path>` reverts files. Inspect and ask before discarding changes.",
    ),
    (
        re.compile(r"\bgit\s+restore\s+(?:\.|:/?|src\b|src-tauri\b|docs\b|scripts\b)"),
        "`git restore` over broad repo paths can discard work. Use a narrower reviewed edit.",
    ),
    (
        re.compile(r"\bgit\s+clean\b(?=[^;&|\n]*-[A-Za-z]*f)(?=[^;&|\n]*-[A-Za-z]*d)"),
        "`git clean -fd` removes untracked files. Ask explicitly before cleaning the worktree.",
    ),
    (
        re.compile(
            r"\brm\s+-[A-Za-z]*r[A-Za-z]*f[A-Za-z]*\s+"
            r"(?:\.|\*|/|~|src\b|src-tauri\b|docs\b|scripts\b|\.agents\b|\.codex\b)"
        ),
        "`rm -rf` over broad repo paths is blocked. Use a precise deletion with review context.",
    ),
)

SHELL_WRITE_RE = re.compile(
    r"((?:cat|printf|echo)\b[^;&|\n]*>\s*|tee\s+(?:-a\s+)?)"
    r"(?:src/|src-tauri/|docs/|scripts/|\.agents/|\.codex/|README\.md|AGENTS\.md)"
)


def command_from_input(payload: dict[str, object]) -> str:
    tool_input = payload.get("tool_input")
    if isinstance(tool_input, dict):
        command = tool_input.get("command")
        if isinstance(command, str):
            return command
    return ""


def deny(reason: str) -> None:
    emit(
        {
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            },
            "systemMessage": reason,
        }
    )


def main() -> None:
    payload = read_hook_input()
    command = command_from_input(payload)
    if not command:
        return

    for pattern, reason in DESTRUCTIVE_RULES:
        if pattern.search(command):
            deny(reason)
            return

    if SHELL_WRITE_RE.search(command):
        emit(
            {
                "systemMessage": (
                    "ABB edit guard: prefer `apply_patch` for manual repo edits so diffs stay "
                    "reviewable and user changes are not overwritten accidentally."
                )
            }
        )
        return


if __name__ == "__main__":
    main()
