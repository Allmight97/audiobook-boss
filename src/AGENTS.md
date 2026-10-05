# Frontend Directives

## Routing

- The engine owns the working session and settings. The frontend renders
  snapshots and sends intents. Owner rules and engine pointers:
  `src/app/AGENTS.md`. View traps: `src/ui/AGENTS.md`.
- IPC adaptation, `tauriClient`, and generated bindings:
  `src/lib/tauri/AGENTS.md`.
- Provider secrets and raw provider payloads stay backend-only.

## Shape

- `src/ui/App.tsx` composes the views and owns the global key bindings.
  `src/main.tsx` mounts `ProductionRoot`.
- `src/styles.css` loads the foundation and owns app-shell layout. Shared
  visual primitives and semantic tokens live in `src/ui/foundation`. Each view
  keeps its layout in its own CSS and reads the public `:root` tokens.
- Audiobook Boss is desktop-only.

## Proof

Verification commands: `scripts/AGENTS.md`.
