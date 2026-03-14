from __future__ import annotations

import json
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DOC_SURFACE_FILES = {
    "README.md",
    "AGENTS.md",
    "docs/fallbacks.md",
    "hooks.json",
    "scripts/check-context-surface.sh",
}
DOC_SURFACE_PREFIXES = (
    ".agents/skills/",
    ".agents/hooks/",
    ".github/ISSUE_TEMPLATE/",
)
IPC_GUARD_PREFIXES = (
    "src-tauri/src/commands/",
    "src/lib/tauri/",
)
IPC_GUARD_FILES = {
    "src-tauri/src/ipc_contract.rs",
    "src/lib/generated/tauri.ts",
}
GENERATED_BINDINGS_PATH = "src/lib/generated/tauri.ts"


def git_output(args: list[str], allow_failure: bool = False) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0 and not allow_failure:
        raise RuntimeError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def changed_paths() -> list[str]:
    output = git_output(["status", "--porcelain", "--untracked-files=all"], allow_failure=True)
    if not output:
        return []

    paths: list[str] = []
    for raw_line in output.splitlines():
        line = raw_line.rstrip()
        if not line:
            continue
        entry = line[3:].strip()
        if " -> " in entry:
            entry = entry.split(" -> ", 1)[1]
        if entry:
            paths.append(entry)
    return paths


def docs_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if entry in DOC_SURFACE_FILES:
            return True
        if any(entry.startswith(prefix) for prefix in DOC_SURFACE_PREFIXES):
            return True
    return False


def ipc_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if entry in IPC_GUARD_FILES:
            return True
        if any(entry.startswith(prefix) for prefix in IPC_GUARD_PREFIXES):
            return True
    return False


def emit(payload: dict[str, object]) -> None:
    print(json.dumps(payload))
