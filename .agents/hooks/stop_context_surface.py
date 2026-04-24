from __future__ import annotations

import subprocess

from common import REPO_ROOT, changed_paths, docs_surface_touched, emit, meaningful_changed_paths


paths = meaningful_changed_paths(changed_paths())
if not docs_surface_touched(paths):
    emit({"continue": True})
    raise SystemExit(0)

result = subprocess.run(
    ["bash", "scripts/check-context-surface.sh"],
    cwd=REPO_ROOT,
    capture_output=True,
    text=True,
    check=False,
)

if result.returncode == 0:
    emit(
        {
            "continue": True,
            "systemMessage": "ABB docs/skill surface check passed.",
        }
    )
else:
    detail = (result.stderr or result.stdout).strip() or "check-context-surface failed"
    emit(
        {
            "continue": False,
            "decision": "block",
            "reason": "ABB docs/skill surface drift detected.",
            "systemMessage": detail,
        }
    )
