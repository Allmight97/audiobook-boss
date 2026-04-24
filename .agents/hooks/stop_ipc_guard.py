from __future__ import annotations

import subprocess

from common import (
    REPO_ROOT,
    changed_paths,
    emit,
    ipc_surface_touched,
    meaningful_changed_paths,
)


paths = meaningful_changed_paths(changed_paths())
if not ipc_surface_touched(paths):
    emit({"continue": True})
    raise SystemExit(0)

result = subprocess.run(
    ["bash", "scripts/check-generated-bindings.sh", "--mode", "local"],
    cwd=REPO_ROOT,
    capture_output=True,
    text=True,
    check=False,
)

if result.returncode == 0:
    emit(
        {
            "continue": True,
            "systemMessage": "ABB IPC guard: generated bindings are current.",
        }
    )
else:
    detail = "\n".join(
        part.strip() for part in (result.stdout, result.stderr) if part.strip()
    )
    emit(
        {
            "continue": False,
            "decision": "block",
            "reason": "ABB IPC generated bindings drift detected.",
            "systemMessage": detail
            or "Run `bun run bindings:sync` or `bun run bindings:check` before finishing.",
        }
    )
