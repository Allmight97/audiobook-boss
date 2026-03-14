from __future__ import annotations

from common import GENERATED_BINDINGS_PATH, changed_paths, emit, ipc_surface_touched


paths = changed_paths()
if not ipc_surface_touched(paths):
    emit({"continue": True})
    raise SystemExit(0)

if GENERATED_BINDINGS_PATH in paths:
    emit(
        {
            "continue": True,
            "systemMessage": "ABB IPC guard: generated bindings changed with the boundary.",
        }
    )
else:
    emit(
        {
            "continue": False,
            "decision": "block",
            "reason": "ABB IPC boundary changed without generated bindings.",
            "systemMessage": (
                "IPC boundary files changed without `src/lib/generated/tauri.ts`. "
                "Run `bun run bindings:sync` or `bun run bindings:check` before finishing."
            ),
        }
    )
