from __future__ import annotations

from common import changed_paths, docs_only_paths, emit, meaningful_changed_paths


paths = meaningful_changed_paths(changed_paths())
if not paths:
    emit({"continue": True})
    raise SystemExit(0)

if docs_only_paths(paths):
    message = "ABB verification lane: docs-only -> run `bash scripts/check-context-surface.sh`."
else:
    message = "ABB verification lane: code/config/build -> run `scripts/checks.sh standard`."

emit(
    {
        "continue": True,
        "systemMessage": message,
    }
)
