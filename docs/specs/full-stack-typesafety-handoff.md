# Full-Stack Typesafety Execution Handoff

## Objective
Implement and land full-stack IPC typesafety on branch `feat/full-stack-typesafety-thread` using `tauri-specta` + `specta`, while preserving current UX behavior and outcomes.

Primary execution issue: #193  
Strategy context: `docs/specs/frontend-framework-evaluation.md`

## Locked Decisions
1. Stay on Tauri.
2. Adopt tauri-specta for all app commands and app events.
3. Commit generated bindings and gate drift.
4. Preserve current error handling semantics (`try/catch` UX) with throw-mode bindings.
5. Keep built-in `tauri://drag-*` events manually typed.
6. Defer framework migration implementation; this is the contract foundation.

## Behavior Invariants
1. Preserve user-visible workflows: import, metadata edit/save, cover art, process, cancel.
2. Preserve command names/event names and key payload field names.
3. No intentional UX/layout redesign in this thread.

## Files Changed / Expected Scope
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/ipc_contract.rs` (new)
- `src-tauri/src/bin/export_bindings.rs` (new)
- `src-tauri/src/commands/audio.rs`
- `src-tauri/src/commands/metadata.rs`
- `src-tauri/src/commands/metadata_lookup/mod.rs`
- `src-tauri/src/commands/system.rs`
- `src-tauri/src/commands/audio_types.rs`
- `src-tauri/src/audio/mod.rs`
- `src-tauri/src/audio/file_list.rs`
- `src-tauri/src/audio/output_path.rs`
- `src-tauri/src/audio/progress/mod.rs`
- `src-tauri/src/audio/settings_encoder.rs`
- `src-tauri/src/commands/metadata_lookup/types.rs`
- `src-tauri/src/errors.rs`
- `src-tauri/src/metadata/mod.rs`
- `src/lib/generated/tauri.ts` (generated)
- `src/lib/bridge.ts`
- `src/lib/behavior-contract.test.ts` (new)
- `src/test/setup.ts`
- `scripts/check-generated-bindings.sh` (new)
- `scripts/checks.sh` (new primary quality gate)
- `package.json`

## Execution Order
1. Add specta/tauri-specta deps and Rust contract builder scaffolding.
2. Annotate commands and derive `specta::Type` for IPC-facing types.
3. Add deterministic TypeScript binding exporter and generated artifact.
4. Add generated-binding drift gate into checks.
5. Refactor frontend bridge to generated bindings with legacy contract compatibility mapping.
6. Add behavior-first regression tests.
7. Update docs/ADR and close #168 via #193 implementation.

## Behavior-First Regression Pack

### Pre/Post baseline smoke run
Run same fixture workflow before/after and compare:
- success/failure outcome
- output file presence
- key metadata fields
- progress completion terminal behavior

### IPC boundary behavior tests
- Test commands through public IPC/bridge surface only.
- Assert behavior and payload semantics, not module internals.

### Event behavior tests
For `processing-progress` and `processing-queue`, assert:
- expected stages appear
- progress percentage remains in valid range
- terminal state arrives (`completed`/`failed`/`cancelled`)
- payload keys remain stable

### Frontend UX-flow tests (outcome-based)
- import
- metadata save
- cover art load
- process
- cancel

### Compatibility guards
- command names unchanged
- event names unchanged
- key payload fields unchanged

## Verification Commands
- `scripts/checks.sh standard`
- `bun run test`
- `bun run bindings:check`

## Fallback Governance
- Register intentional fallbacks in `docs/engineering/fallback-register.md`.
- All fallback markers must include issue + sunset metadata and pass `scripts/check-fallback-policy.sh`.
- Migration-bound fallback for bridge wrapper is tracked in #203.

## Commit Grouping Rule
Commit in coherent units (contract builder, derives, exporter, bridge mapping, tests, docs).  
Each commit should be reviewable and independently meaningful.

## Rollback Plan
If tauri-specta integration blocks progress:
1. restore legacy `generate_handler![]` registration in `src-tauri/src/lib.rs`
2. remove specta dependencies and generated bindings
3. keep ADR/issue notes documenting blocker and fallback

## Next Phase Intent
After this contract foundation lands and is stable, proceed with Tailwind + framework migration track (Svelte-first direction per `docs/specs/frontend-framework-evaluation.md`).
