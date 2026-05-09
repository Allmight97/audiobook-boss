from __future__ import annotations

import re

from common import emit, read_hook_input


SKILL_RULES: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "contract-guardrails",
        re.compile(r"\b(ipc|contract|binding|bindings|generated|payload|event|tauri\.ts)\b", re.I),
    ),
    (
        "audiobook-metadata",
        re.compile(
            r"\b(metadata|tag|tags|m4b|mp4|audiobookshelf|abs|plex|apple books|narrator|series)\b",
            re.I,
        ),
    ),
    (
        "path-security-validation",
        re.compile(r"\b(path|paths|output dir|output directory|file write|writes?|delete|sanitize)\b", re.I),
    ),
    (
        "job-registry-and-progress",
        re.compile(r"\b(queue|queued|job registry|job|jobs|cancel|cancellation|progress)\b", re.I),
    ),
    (
        "tauri-command-conventions",
        re.compile(r"\b(tauri command|command handler|invoke|State<|tauri::command)\b", re.I),
    ),
    (
        "dependency-maintenance",
        re.compile(r"\b(dependency|dependencies|bun|cargo|crate|npm|package|lockfile|toolchain|supply-chain)\b", re.I),
    ),
)


def prompt_from_input(payload: dict[str, object]) -> str:
    prompt = payload.get("prompt")
    if isinstance(prompt, str):
        return prompt
    return ""


def main() -> None:
    prompt = prompt_from_input(read_hook_input())
    if not prompt:
        emit({"continue": True})
        return

    matched = [skill for skill, pattern in SKILL_RULES if pattern.search(prompt)]
    if not matched:
        emit({"continue": True})
        return

    skills = ", ".join(f"`{skill}`" for skill in matched)
    emit(
        {
            "continue": True,
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": (
                    "ABB skill-routing hint: this prompt appears to touch "
                    f"{skills}. Load the matching skill before editing that boundary."
                ),
            },
        }
    )


if __name__ == "__main__":
    main()
