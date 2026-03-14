from __future__ import annotations

from common import changed_paths, docs_surface_touched, emit


paths = [entry for entry in changed_paths() if not entry.startswith(".artifacts/")]
if not paths:
    emit({"continue": True})
    raise SystemExit(0)

if all(docs_surface_touched([entry]) for entry in paths):
    message = "ABB verification lane: docs-only -> run `bash scripts/check-context-surface.sh`."
else:
    message = "ABB verification lane: code/config/build -> run `scripts/checks.sh standard`."

emit(
    {
        "continue": True,
        "systemMessage": message,
    }
)
