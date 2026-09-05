# Frontend Directives

## Routing

- Application interfaces, session state, and workflow lifetime follow
  `src/app/AGENTS.md`; read it when changing frontend session truth.
- Runtime command/event/plugin adaptation follows `src/lib/tauri/AGENTS.md`.
  UI/runtime callers use `tauriClient`; generated invokers stay inside that
  boundary. Regenerate `src/lib/generated/tauri.ts` through the binding scripts.
- Metadata intent adaptation stays at the Tauri boundary; canonical validation
  and normalization stay with Rust Metadata Outcome. For metadata-save
  lifecycle display, also read `src/app/workOperations/AGENTS.md`.
- For Effect workflow or kernel changes, read `src/lib/effect/AGENTS.md`.
- Durable preference hydration and persistence belong to `src/app/appSettings`.
  The remaining UI persistence exception is documented in
  `src/ui/appSettings/AGENTS.md`; use the Settings owner for new callers.
- Remote acquisition belongs to `src/app/remoteSource`; its Solid dialog is
  under `src/ui/remoteSource`. Materialized audio enters through Input's public
  strip. Provider secrets and raw provider payloads stay backend-only.

## UI And State

- Keep business logic in TypeScript owners; Solid views render owner state and
  dispatch semantic intent. Capability accept/reject facts come from their
  Rust owner, including encoder and concurrency settings.
- Keep `src/ui/App.tsx` and `src/main.tsx` declarative composition surfaces.
- `src/styles.css` loads the foundation and owns app-shell layout. Shared
  visual primitives and semantic tokens belong to `src/ui/foundation`; read
  its `AGENTS.md` when changing shared visual behavior.
- Owner layout lives in that owner's CSS. Consume public semantic tokens;
  imports of another owner's CSS or foundation internals bypass ownership.
- Update the design lab (`lab.html` + `src/lab/`, Vite dev only) when
  changing foundation tokens or primitives.
- Audiobook Boss is desktop-only. Alternate viewport review applies when the
  task explicitly requests it.

## Proof

Use `scripts/AGENTS.md` for focused frontend, type, and boundary checks.
For UI changes, inspect the rendered behavior when layout, interaction, or
visual judgment is part of acceptance. Add tests for concrete behavior or
integration risk under root's test-value bar.
