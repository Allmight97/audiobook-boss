# Pattern Cleanup Roadmap — Active Reference

Status: temporary active roadmap reference.
Tracker: GitHub issue #361.
Update rule: update this file only when the implementation strategy or owner
gates change. GitHub issue #361 owns live status.
Cleanup: delete this file when #361 closes, or distill only enduring ownership
rules into `docs/system-map.md`, `docs/ubiquitous-language.md`,
`docs/api-map.md`, or nearest `AGENTS.md` files.

## North Star

Finish #361 by removing dead patterns, reach-through imports, duplicated
lifecycle truth, untyped runtime-boundary drift, and false public surfaces.
Prefer owner-coherent work over small PR theater. Split only when generated
bindings, review feedback, or owner collision creates a real technical reason.

## Current Branch Strategy

Use `feat/pattern-roadmap-consolidation` as the consolidated implementation
branch. Start at the frontend boundary because it is the current shallowest
entry point, then pull adjacent cleanup forward when the same owner is already
open.

GitHub issue #361 remains the tracker. This document is the context-restoration
packet for long implementation sessions.

## Current Branch Evidence

Validated on 2026-06-07 against the consolidated roadmap branch.

Repo evidence:

- PR #362 is merged into `main`; WorkRuntime backend terminal truth landed.
- Work Center consumes WorkRuntime operation snapshots and does not subscribe to
  direct `processing-progress` events.
- Backend processing progress events update WorkRuntime operation snapshots
  before the frontend observes them.
- `src/ui/statusPanel/domain/stateMachine.ts` ignores `operation_id` events,
  preserving Status Panel as a direct foreground lifecycle consumer.
- Preview execution still uses `process_audiobook_files` through
  `src/ui/statusPanel/processingWorkflow.ts`.
- Metadata batch save still emits processing lifecycle events from
  `src-tauri/src/commands/metadata/save_batch.rs`.
- `src/App.svelte` renders `EncodingWorkbenchIsland`; that workbench composes
  `src/ui/encoderPanel/EncoderWorkbenchIsland.svelte`.
- The dead encoder panel shell and dependency-only Effect smoke test are
  deleted.
- Encoder UI uses the output-panel Public API Strip instead of private
  output-panel modules.
- Runtime settings capability loading is owned by
  `src/ui/runtimeSettingsCapabilities.svelte.ts`.
- `src/ui/jobControls/index.ts` is the job-controls public strip entry point.
- `normalizeEncoderAvailability` is removed.
- `src-tauri/src/processing/run.rs` is split into dispatch, job, options, and
  validation owner modules.
- Remote-source UI state/account/workflow/helpers are split from
  `RemoteSourceAcquireDialog.svelte`; backend session lifecycle and Audible
  probe code have owned modules.
- Output-artifact false public/test-only exports were stripped where callers did
  not need the public strip.

Library/source evidence:

- Installed frontend stack: Svelte 5.55.9, Effect 3.21.2, Tauri API 2.10.1,
  Vitest 4.1.5, TypeScript 6.0.3 in `package.json` and `bun.lock`.
- Installed Rust Tauri stack: `tauri` 2.10.3 plus current Specta/
  `tauri-specta` bindings in `src-tauri/Cargo.toml`.
- Svelte docs validate `.svelte.ts` rune modules as real shared reactive state
  modules; ABB should still treat ownership as an app boundary decision.
- Effect docs validate workflows, typed failures, services/context, and layers
  as real library concepts; ABB should keep Effect private to owning workflows
  unless a boundary decision changes.
- Tauri docs validate commands/events as the external IPC vocabulary; ABB still
  centralizes frontend use through `src/lib/tauri/*`.

## Work Order

### 1. Frontend Boundary Cleanup

Purpose: remove dead frontend shell code and repair UI module ownership leaks.

Implement:

- Delete `src/ui/encoderPanel/EncoderPanelIsland.svelte`.
- Retarget or delete tests that preserve the deleted shell.
- Replace encoder imports from `../outputPanel/state.svelte` and
  `../outputPanel/preview` with the output-panel Public API Strip.
- Make `src/ui/runtimeSettingsCapabilities.svelte.ts` own load/cache/refresh
  semantics for runtime settings capabilities.
- Remove duplicate sequential capability loads from app settings hydration,
  encoder defaults/init, and job controls.
- Move `src/ui/jobControls.ts` into `src/ui/jobControls/index.ts` if imports
  remain stable through the folder public surface.
- Update nearest `AGENTS.md` files if Public API Strips intentionally change.

Safe pull-forward:

- Delete `src/effect-smoke.test.ts` after focused frontend workflow tests remain
  green.
- Remove unused `normalizeEncoderAvailability` if `rg` still finds no caller.

Do not pull forward:

- Output-artifact test-only export cleanup; keep that with module cohesion.
- Broad Svelte syntax cleanup unless a touched component requires it.
- Unrelated loose UI files that are not on the touched ownership path.

### 2. Work Runtime Lifecycle Closure

Purpose: make WorkRuntime and Work Center the single background-operation truth
for accepted processing work, and classify every direct foreground survivor.

Deletion gate:

- Work Center renders backend-authored operation snapshots without applying
  direct `processing-progress` events.
- Backend operation snapshots carry the progress detail needed by Work Center.
- Status Panel is either a migrated operation consumer or a deliberately
  retained foreground/direct-execution adapter.
- Preview execution and `process_audiobook_files` survival are explicit.
- Metadata save is migrated to WorkRuntime or explicitly retained as a
  metadata-save adapter with #307 updated.
- Direct processing commands/events/fixtures are removed only when no runtime
  consumer remains.
- `docs/api-map.md` no longer points at deleted lifecycle specs or stale
  overlay language.

Do not:

- Let UI invent terminal status.
- Reintroduce terminal classification outside the canonical
  `abb_processing_core::classify_run_terminal` path.
- Grow `processing/run.rs`, Work Center model/state, or Status Panel lifecycle
  modules without a split or a documented owner reason.

### 3. Runtime Boundary Contract Maintenance

Purpose: make frontend runtime-boundary tests and mocks fail when generated IPC
truth drifts, without maintaining duplicate untyped command mirrors.

Implement:

- Add a standalone frontend type-validation route.
- Type `src/test/setup.ts` command mocks against generated response shapes or
  generated aliases.
- Keep meaningful `tauriClient` Public API Strip tests.
- Remove duplicate generated-truth mirror lists when generated bindings plus
  strip tests already prove the contract.
- Align `metadataPatch` / `metadataIntent` naming at the handwritten adapter
  boundary where useful.
- Update `scripts/AGENTS.md` if the typecheck route becomes canonical proof.

Do not:

- Replace generated bindings with hand-authored types.
- Remove `src-tauri/src/ipc_contract.rs` registration lists.
- Add parser/tooling complexity for generated output without current drift
  evidence.

### 4. Module Cohesion Decomposition

Purpose: split the largest pressure points by ownership so future remote
acquisition, processing, and artifact work does not keep adding branches to
overgrown coordinators.

Implement after lifecycle cleanup unless an already-open module creates
immediate risk:

- Refresh current LOC/evidence before editing.
- Split `src/ui/remoteSource/RemoteSourceAcquireDialog.svelte` into workflow,
  state/controller, and rendering pieces.
- Deepen Audible provider private modules around acquisition orchestration,
  materialization handoff, Supplemental PDF policy, diagnostics/redaction, and
  cleanup.
- Extract RemoteSourceRuntime cancellation/session cleanup/purge behavior when
  current code supports a cleaner owner.
- Split `src-tauri/src/processing/run.rs` after Work Runtime lifecycle closure
  reduces the relevant paths.
- Audit `src-tauri/src/output_artifact/mod.rs` public-strip exports with
  `#[allow(unused_imports)]` and keep only exports with a real public/test
  boundary reason.
- Remove or productionize false-green/test-only paths from #304/#355 while the
  same modules are open.

Do not:

- Run live Audible network tests as roadmap proof.
- Expose provider secrets or raw provider payloads to UI/log surfaces.
- Split pure core crates only to satisfy line count.

## Tracker Rules

- Update #361 at the start of each roadmap planning session and when the branch
  is pushed for review.
- Keep #361 current-state only; do not duplicate detailed implementation state
  in issue comments.
- Close or update related issues only when the owning implementation evidence is
  in the branch: #348, #356, #359, #304, #355, #307, and #341/#354.
- Deferred material work needs owner, trigger, and reason.

## Verification Menu

Frontend boundary:

- `bun run test -- src/ui/__tests__/encoderPanel-behavior.test.ts`
- `bun run test -- src/ui/__tests__/encoderPanel-native-warning.test.ts`
- `bun run test -- src/ui/__tests__/encodingWorkbench-island.test.ts`
- `bun run test -- src/ui/__tests__/outputPanel-store-driven.test.ts`
- `bun run test -- src/ui/__tests__/runtimeSettingsCapabilities.test.ts`
- `bun run test -- src/ui/__tests__/jobControls-info.test.ts`
- `bun run test -- src/ui/appSettings/appSettings.test.ts`

Work Runtime lifecycle:

- `cargo nextest run -p abb-processing-core`
- `cargo nextest run -p audiobook-boss --lib work_runtime`
- `bun run test -- src/ui/workCenter/__tests__/*.test.ts`
- Status Panel, preview, and metadata-save focused Vitest files when touched.
- `bash scripts/check-generated-bindings.sh --mode local` when IPC shapes
  change.

Runtime boundary:

- `bun run typecheck`.
- `bun run test -- src/lib/behavior-contract.test.ts src/lib/tauri-public-api.contract.test.ts src/lib/tauri-client.test.ts`
- `bun scripts/check-tauri-runtime-boundary.ts`
- `bash scripts/check-generated-bindings.sh --mode local` when generated truth
  changes.

Module cohesion:

- `cargo nextest run -p abb-remote-source-core`
- `cargo nextest run -p audiobook-boss --lib remote_source`
- `cargo nextest run -p audiobook-boss --test all_tests <focused filters>`
- `bun run test -- src/ui/remoteSource/<focused tests>`
- Core crate package-selected tests for pure splits.

Always:

- `git diff --check`

## Closeout

When #361 is done:

- Delete this roadmap packet.
- Update #361 with the final closure summary.
- Distill only lasting ownership rules into durable docs.
- Do not leave completed workblock specs or historical roadmap narration in the
  repo.
