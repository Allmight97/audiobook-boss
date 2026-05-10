from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DOCS_ONLY_FILES = {
    "README.md",
    "AGENTS.md",
    "docs/fallbacks.md",
    "docs/api-map.md",
    "src/AGENTS.md",
}
DOCS_ONLY_PREFIXES = (
    ".agents/skills/",
    "docs/specs/",
    ".github/ISSUE_TEMPLATE/",
)
DOCS_ARTIFACT_PREFIXES = (
    ".artifacts/",
)
DOC_SURFACE_FILES = {
    *DOCS_ONLY_FILES,
    ".agents/hooks.json",
    ".codex/hooks.json",
    "hooks.json",
    "scripts/check-context-surface.sh",
}
DOC_SURFACE_PREFIXES = (
    *DOCS_ONLY_PREFIXES,
    ".agents/hooks/",
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
FALLBACK_SURFACE_FILES = {
    "docs/fallbacks.md",
    "scripts/check-fallback-policy.sh",
}
FALLBACK_SURFACE_PREFIXES = (
    "src-tauri/src/audio/",
    "src-tauri/src/metadata/",
    "src-tauri/src/commands/audio_processing/",
    "src-tauri/src/commands/metadata/",
    "src/lib/tauri/",
)
UI_SURFACE_PREFIXES = (
    "src/App.svelte",
    "src/ui/",
    "src/lib/tauri/",
)
EPHEMERAL_PATH_EXACT = {
    ".DS_Store",
}
EPHEMERAL_PATH_PREFIXES = (
    ".artifacts/",
    ".pytest_cache/",
    ".mypy_cache/",
    ".ruff_cache/",
    "__pycache__/",
)
FALLBACK_KEYWORD_RE = re.compile(
    r"FALLBACK\[FB-\d{3}\]|falling back|fall back|back-compat|"
    r"backward compatibility|legacy compatibility|retained for compatibility|shim",
    re.IGNORECASE,
)
EPHEMERAL_PATH_SUFFIXES = (
    ".pyc",
    ".pyo",
    ".pyd",
    "~",
)


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


def is_ephemeral_path(entry: str) -> bool:
    if entry in EPHEMERAL_PATH_EXACT:
        return True
    if any(entry.startswith(prefix) for prefix in EPHEMERAL_PATH_PREFIXES):
        return True
    if any(entry.endswith(suffix) for suffix in EPHEMERAL_PATH_SUFFIXES):
        return True
    return False


def meaningful_changed_paths(paths: list[str]) -> list[str]:
    return [entry for entry in paths if not is_ephemeral_path(entry)]


def docs_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if entry.endswith("/AGENTS.md"):
            return True
        if entry in DOC_SURFACE_FILES:
            return True
        if any(entry.startswith(prefix) for prefix in DOC_SURFACE_PREFIXES):
            return True
    return False


def docs_only_paths(paths: list[str]) -> bool:
    if not paths:
        return False
    for entry in paths:
        if any(entry.startswith(prefix) for prefix in DOCS_ARTIFACT_PREFIXES):
            continue
        if entry.endswith("/AGENTS.md"):
            continue
        if entry in DOCS_ONLY_FILES:
            continue
        if any(entry.startswith(prefix) for prefix in DOCS_ONLY_PREFIXES):
            continue
        return False
    return True


def ipc_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if entry in IPC_GUARD_FILES:
            return True
        if any(entry.startswith(prefix) for prefix in IPC_GUARD_PREFIXES):
            return True
    return False


def fallback_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if entry in FALLBACK_SURFACE_FILES:
            return True
        if any(entry.startswith(prefix) for prefix in FALLBACK_SURFACE_PREFIXES):
            if file_contains(entry, FALLBACK_KEYWORD_RE):
                return True
    return False


def ui_surface_touched(paths: list[str]) -> bool:
    for entry in paths:
        if any(entry == prefix or entry.startswith(prefix) for prefix in UI_SURFACE_PREFIXES):
            return True
    return False


def file_contains(entry: str, pattern: re.Pattern[str]) -> bool:
    path = REPO_ROOT / entry
    if not path.is_file():
        return False
    try:
        return pattern.search(path.read_text(encoding="utf-8", errors="ignore")) is not None
    except OSError:
        return False


def read_hook_input() -> dict[str, object]:
    raw = sys.stdin.read().strip()
    if not raw:
        return {}
    try:
        value = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    if isinstance(value, dict):
        return value
    return {}


def run_command(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def emit(payload: dict[str, object]) -> None:
    print(json.dumps(payload))
