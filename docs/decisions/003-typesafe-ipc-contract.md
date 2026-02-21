# ADR-003: Typesafe IPC Contract with tauri-specta

**Status:** accepted  
**Date:** 2026-02-10  
**Issue:** #193

## Context
The app’s Rust↔TS IPC contract was maintained manually across command strings, payload types, and event payload docs. This created drift risk, including during the now-complete Svelte migration where stable contracts reduced rewrite risk, and now serves as an ongoing contract-stability guardrail.

## Decision
Adopt `tauri-specta` + `specta` as the IPC contract source of truth:
- Annotate Tauri commands with `#[specta::specta]`.
- Derive `specta::Type` for IPC-facing Rust types.
- Generate and commit TypeScript bindings (`src/lib/generated/tauri.ts`).
- Gate checks with generated-binding drift detection.
- Preserve current UX behavior by mapping generated nullability and event payloads through the `tauriClient` boundary (`src/lib/tauri/client.ts` + `src/lib/tauri/normalizers.ts`), while keeping a strict single-shape boundary input contract (no dual-key alias fallbacks).

## Consequences
### Pros
- Stronger compile-time safety across Rust and TypeScript IPC boundaries.
- Fewer runtime regressions from command/event contract drift.
- Ongoing contract-stability guardrail for post-migration frontend evolution.

### Cons
- Added build/tooling dependency on tauri-specta/specta.
- Generated output introduces contract-format churn risk when dependency versions change.
- `tauriClient` boundary now carries compatibility mapping logic for legacy optional-vs-null semantics.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep manual typed wrapper only | Does not remove source-of-truth duplication or drift class. |
| TauRPC | More opinionated shift than needed for current contract hardening objective. |
| tauri-bindgen (WIT/IDL) | Higher setup/migration overhead for current app scale and timeline. |
| Migrate to Electron first | Foundation/runtime decision is orthogonal; contract safety was immediate lower-risk win on Tauri path. |
