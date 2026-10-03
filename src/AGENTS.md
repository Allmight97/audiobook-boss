# Frontend Directives

## Routing

- The Rust engine owns the working session and settings, and new product
  rules go there; the frontend renders snapshots and sends intents. Application interfaces,
  the engine link, and workflow lifetime follow `src/app/AGENTS.md`.
- Runtime command/event/plugin adaptation follows `src/lib/tauri/AGENTS.md`.
  UI/runtime callers use `tauriClient`; generated invokers stay inside that
  boundary. Regenerate `src/lib/generated/tauri.ts` through the binding scripts.
- The metadata batch save runs as a WorkRuntime operation rendered by Work
  Center; read `src/app/workOperations/AGENTS.md` when changing its display.
- Settings acceptance and persistence belong to the engine
  (`crates/abb-engine/src/app_settings/AGENTS.md`); the Settings dialog
  follows `src/app/appSettings/AGENTS.md`.
- The remote-source dialog belongs to `src/app/remoteSource` and
  `src/ui/remoteSource`; the engine imports acquired audio into the session
  and decides when downloads go. Provider secrets and raw provider payloads
  stay backend-only.

## UI And State

- Solid views render owner state and dispatch semantic intent. Capability
  accept/reject facts come from their Rust owner, including encoder and
  concurrency settings.
- Keep `src/ui/App.tsx` and `src/main.tsx` declarative composition surfaces.
- `src/styles.css` loads the foundation and owns app-shell layout. Shared
  visual primitives and semantic tokens belong to `src/ui/foundation`; read
  its `AGENTS.md` when changing shared visual behavior.
- Owner layout lives in that owner's CSS. Consume public semantic tokens;
  imports of another owner's CSS or foundation internals bypass ownership.
- Audiobook Boss is desktop-only. Alternate viewport review applies when the
  task explicitly requests it.

## Proof

Use `scripts/AGENTS.md` for focused frontend, type, and boundary checks.
For UI changes, inspect the rendered behavior when layout, interaction, or
visual judgment is part of acceptance. Add tests for concrete behavior or
integration risk under root's test-value bar.
