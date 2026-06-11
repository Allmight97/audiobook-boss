# AppEffect Kernel Guidance

## Scope

- Applies to ABB's local Effect kernel and conventions under `src/lib/effect/`.
- Keep Effect as workflow-owner infrastructure, not a general UI dependency.

## Workflow Owner Shape

Use `AppEffect` for frontend workflow owners that coordinate async work,
dependencies, typed failures, cleanup, cancellation/lifetime handoff, or
multi-service orchestration.

Keep Effect private to workflow owners:

- Public UI/runtime entrypoints expose Promise-returning functions or existing
  synchronous wrappers where callers already rely on them.
- Workflow owners expose a local service interface, service tag, live layer
  (co-located in the workflow file with `satisfies`), typed errors, program,
  and Promise bridge. `ProcessingWorkflow` keeps live deps in
  `processingWorkflow.deps.ts` (dynamic-imported) to preserve the file-list
  cycle break.
- Dependencies are injected through service objects so tests can provide fake
  layers without Svelte rendering or live Tauri.
- State and event outputs stay explicit in the owner contract.
- Public API Strip impact is stated by the wrapper: if callers do not need new
  symbols or changed return types, keep the existing public API stable.
- Scenario tests prove visible workflow outcomes, cleanup/lifetime handoff,
  cancellation behavior where relevant, terminal results, and typed failure
  handling.

## Workflow Harness Helpers

Workflow owners map infrastructure failures into owner-specific tagged errors
through the shared kernel helpers:

- `workflowTryPromise` for async service calls
- `workflowTrySync` for synchronous service calls

Each owner keeps its own `*WorkflowFailed` tagged error and `workflowFailure`
factory. Do not reintroduce local `Effect.tryPromise` / `Effect.try` blocks in
owner files unless the failure mapping is genuinely unique (for example
`ProcessingWorkflow` normalizes `AppError` before constructing its failure).

## Fake-Layer Harness Shape

Workflow tests should run the Effect program directly with fake services:

- Build a small `makeHarness` helper in the owner test file.
- Provide dependencies through the owner service layer, not through Svelte
  rendering or live Tauri.
- Prefer assertions on public workflow outcomes: visible status, state writes,
  cleanup, terminal results, user-safe errors, and typed workflow errors.
- Keep fake services close to the owner test unless a second owner needs the
  same harness shape.
- Use live layers only from UI/runtime entrypoints; tests for owners should
  import the service layer helper and call the program or Promise bridge.

## Workflow Service Catalog

Current owners and focused test examples. Use `bun run test -- <files>` for
frontend focus when proving touched workflow behavior; the table names focused
Vitest selections for local diagnosis.

| Owner | Coordinates | Service families | Public entrypoints | Terminal outcomes and focused tests |
| --- | --- | --- | --- | --- |
| `ProcessingWorkflow` | Processing request composition, output-plan review, metadata staging, listener startup, process IPC, cancellation, terminal status. | File list, metadata form/state, encoder/output panel Public API Strips, job controls, status feedback, `tauriClient`. | `startProcessing(...)` via status-panel runtime. | Approved processing, blocked review, cancellation, failed command. `bun run test -- src/ui/statusPanel/__tests__/processingWorkflow.test.ts` |
| `MetadataSaveWorkflow` | File availability, save lifecycle, draft persistence, pending intent filtering, batch save, cleanup. | File list, metadata form/state, status panel Public API Strip, `tauriClient`. | `saveMetadataFromUI()` through `src/ui/core/actions.ts`. | No files, processing active, save busy, validation failure, no-op, partial/failed batch, typed failure. `bun run test -- src/ui/core/__tests__/metadataSaveWorkflow.test.ts` |
| `MetadataLookupWorkflow` | Lookup queue, search, result apply, queue advancement, lookup-result cover-art replacement. | Metadata lookup state, file selection/list, metadata form/state, output/tag preview, cover art, `tauriClient`. | `runMetadataLookupWorkflow(...)` behind metadata lookup UI actions. | Open/search/apply/skip, queue completion, cover-art success/failure, typed failure. `bun run test -- src/ui/metadataLookup/__tests__/metadataLookupWorkflow.test.ts` |
| `OutputPlanWorkflow` | Output preview, stale preview handling, preflight, collision review. | Output panel state, metadata, collision dialog, `tauriClient`. | `runOutputPathPreviewWorkflow(...)`, `runOutputPlanReviewWorkflow(...)`. | Preview success/missing dir/stale/failure, approved/block/cancel/reviewed preflight. `bun run test -- src/ui/outputPanel/__tests__/outputPlanWorkflow.test.ts` |
| `ImportAnalysisWorkflow` | Picker/drop import, supported-path filtering, order-lock checks, analysis, metadata draft staging, append/error cleanup. | File import state, file list, status panel Public API Strip, `tauriClient`. | `runImportAnalysisWorkflow(...)` from import handlers. | Locked, picker cancel/failure, unsupported drop, analysis/staging failure, append success, duplicate-only. `bun run test -- src/ui/fileImport/__tests__/importAnalysisWorkflow.test.ts` |
| `ProcessingCancellationWorkflow` | Cancel-all pending lifecycle and per-job cancellation failure reporting. | Status feedback and `tauriClient.cancelProcessing`. | `runProcessingCancellationWorkflow(...)` through status-panel controller. | Cancel-all success/failure, per-job cancel success/failure. `bun run test -- src/ui/statusPanel/__tests__/processingCancellationWorkflow.test.ts` |

Public API Strip impact for these owners is intentionally narrow: callers keep
existing UI/runtime Promise or synchronous wrapper shapes unless a milestone
explicitly accepts a public API change.

## Future Workflow Ingress

- New frontend work that coordinates multiple real boundaries should start with
  an explicit workflow owner, co-located service contract + live layer, and
  fake-layer tests.
- Plain local UI state, pure transforms, and single-boundary event handlers can
  stay vanilla TypeScript when Effect would not clarify ownership or testing.
- Manual cover-art file, URL, drop, and clear loading remains vanilla in
  `src/ui/coverArt/index.ts` until that flow proves it needs a workflow owner.
