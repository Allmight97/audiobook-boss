# Frontend Boundary Cleanup — Active Spec

Status: temporary active spec.
Tracker: GitHub issue #361, WB-B.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: frontend UI modules respect their Public API Strips, dead workbench
components are gone, and runtime-capability/module-organization cleanup reduces
cross-panel ownership drift.

Acceptance signal: encoding workbench renders through one canonical component,
no encoder module imports output-panel private state, runtime capability loading
has one clear owner, and loose UI files that shadow folder modules are either
moved into owners or explicitly justified.

## Progress

- [x] 2026-06-07: planning refreshed after PR #362 landed. GitHub issue #361
  was updated to mark PR #362 merged and to keep WB-A open until repo evidence
  supports deleting `docs/specs/work-runtime-lifecycle-retirement.md`.
- [ ] Delete the orphaned encoder panel component and tests.
- [ ] Replace output-panel private-state reach-through with the output panel
  Public API Strip.
- [ ] Resolve runtime-capability hydration ownership from #348.
- [ ] Reconcile loose UI module/folder organization where it affects ownership
  clarity.

## Surprises & Discoveries

- Observation: `EncoderPanelIsland.svelte` is retained by tests after runtime UI
  moved to `EncoderWorkbenchIsland.svelte`.
  Evidence: `src/ui/encodingWorkbench/EncodingWorkbenchIsland.svelte`,
  `src/ui/__tests__/encoderPanel-*.test.ts`.
- Observation: both encoder islands import `outputPanelState` from another
  module's private cluster.
  Evidence: `src/ui/encoderPanel/EncoderWorkbenchIsland.svelte`,
  `src/ui/encoderPanel/EncoderPanelIsland.svelte`,
  `src/ui/outputPanel/AGENTS.md`.
- Observation: runtime capability loading can be initiated from app settings,
  encoder panel, and job controls.
  Evidence: GitHub issue #348.
- Observation: `src/ui/encoderPanel/logic.ts` imports `updateEstimatedSize`
  from `../outputPanel/preview`, another output-panel private-cluster
  reach-through. The output panel index already exports `updateEstimatedSize`.
  Evidence: `src/ui/encoderPanel/logic.ts`,
  `src/ui/outputPanel/index.ts`, `src/ui/outputPanel/AGENTS.md`.
- Observation: `src/ui/jobControls.ts` is both a touched runtime capability
  owner and a loose file shadowing `src/ui/jobControls/`.
  Evidence: `src/ui/jobControls.ts`, `src/ui/jobControls/JobControlsIsland.svelte`.
- Observation: the safe trivial cleanup candidates are
  `src/effect-smoke.test.ts` and the unused
  `normalizeEncoderAvailability` export. `experiments/` is not a tracked repo
  item in the current checkout, and output-artifact test-only exports belong
  with the WB-D Rust module-boundary cleanup.
  Evidence: GitHub issue #361, `src/effect-smoke.test.ts`,
  `src/lib/tauri/normalizers.ts`, `docs/specs/module-cohesion-decomposition.md`.

## Decision Log

- Decision: use a consolidated roadmap branch if the code remains reviewable
  by owner-scoped commits and targeted checks.
  Rationale: WB-B is the next entry point, but current repo evidence says
  WB-A lifecycle closure is still real work. A single branch can finish the
  roadmap without staging process theater; split only if review feedback,
  generated-binding churn, or owner collision becomes an implementation risk.
  Date: 2026-06-07.
- Decision: do not delete
  `docs/specs/work-runtime-lifecycle-retirement.md` at branch start.
  Rationale: Work Center still consumes legacy `processing-progress`, and
  Status Panel/preview/metadata-save lifecycle paths still need final
  classification. Delete the spec only after those repo facts are changed or
  explicitly recorded as deliberate adapters.
  Date: 2026-06-07.
- Decision: include `src/effect-smoke.test.ts` and unused runtime-boundary
  normalizer cleanup in this branch when verification remains local.
  Rationale: they are trivial, current-truth cleanups that reduce later PR
  churn. Output-artifact test-only export cleanup stays with WB-D because it
  touches a Rust module-boundary surface.
  Date: 2026-06-07.
- Decision: treat encoder cleanup, output-panel reach-through, and runtime
  capability ownership as one frontend-boundary workblock.
  Rationale: the same user-facing controls expose ownership drift between
  encoder, output, settings, and status surfaces.
  Date: 2026-06-07.
- Decision: include small UI module-organization cleanup when it is on the
  touched ownership path.
  Rationale: this closes trivial pattern smells without creating cleanup-only
  PRs.
  Date: 2026-06-07.

## Context And Orientation

- Owning frontend surfaces: `src/ui/encoderPanel/`, `src/ui/outputPanel/`,
  `src/ui/appSettings/`, `src/ui/jobControls.ts`,
  `src/ui/runtimeSettingsCapabilities.svelte.ts`.
- Related issue: #348 runtime capability hydration ownership.
- Related pattern findings: DP2, PS1, PS6.
- Terms from `docs/ubiquitous-language.md`: Public API Strip, Private Cluster,
  Reach-Through, Runtime Settings Capabilities.

## Scope And Constraints

In scope:

- Delete `EncoderPanelIsland.svelte` if `EncoderWorkbenchIsland.svelte` is the
  canonical runtime component.
- Delete or retarget tests that only preserve the old island.
- Use `src/ui/outputPanel` exports instead of importing
  `outputPanel/state.svelte.ts`.
- Add or adjust output-panel strip exports only when they own real output-panel
  truth.
- Choose one runtime capability lifecycle owner:
  - app settings/control-plane loads and passes capabilities; or
  - runtime capability store owns load/cache/refresh semantics.
- Update local `AGENTS.md` files if Public API Strips intentionally change.
- Reconcile loose `src/ui/*.ts` siblings versus folder modules when they are on
  the touched ownership path.

Out of scope:

- Broad Svelte syntax cleanup.
- Visual redesign of the encoder/output/status panels.
- Backend encoder/toolchain policy changes.
- Rewriting metadata form behavior unless module organization requires a narrow
  move.

Constraints:

- UI labels and hints stay frontend-owned; accept/reject capability facts stay
  backend-owned.
- Do not mirror encoder state into output or status panels.
- Do not add new public exports solely to avoid moving logic to the owner.
- Do not preserve tests whose only value is keeping a dead component alive.

## Plan Of Work

Implementation order:

1. Tracker and spec alignment.
   - Keep GitHub issue #361 current at the start of the work session and when
     the branch is pushed for review.
   - Keep this spec as the tactical plan for the frontend-boundary entry point.
   - Keep `docs/specs/work-runtime-lifecycle-retirement.md` active until the
     Work Runtime deletion gate is satisfied.

2. Canonical encoder shell.
   - Confirm `src/App.svelte` renders `EncodingWorkbenchIsland`, which composes
     `src/ui/encoderPanel/EncoderWorkbenchIsland.svelte`.
   - Delete `src/ui/encoderPanel/EncoderPanelIsland.svelte`.
   - Retarget encoder behavior/native-warning/island tests from
     `EncoderPanelIsland` to the canonical workbench component or delete tests
     that only preserve the dead shell.
   - Update `src/ui/__tests__/svelte-event-directives.test.ts` and
     `src/ui/encoderPanel/AGENTS.md` so no ownership doc lists the deleted file.

3. Output panel Public API Strip repair.
   - Replace encoder component imports of `../outputPanel/state.svelte` with
     output-panel index access. Prefer existing `getState` unless a narrower
     estimated-size reader is needed for Svelte reactivity.
   - Replace `src/ui/encoderPanel/logic.ts` import from
     `../outputPanel/preview` with `../outputPanel`.
   - Update `src/ui/outputPanel/AGENTS.md` only if the Public API Strip changes.

4. Runtime settings capability ownership.
   - Make `src/ui/runtimeSettingsCapabilities.svelte.ts` the owner of
     load/cache/refresh semantics for runtime settings capabilities.
   - Keep pending-load de-duplication, add completed-result caching, and expose
     an explicit refresh route for future settings changes.
   - Remove duplicate sequential loads from `applyEncodingDefaults`,
     `initializeEncoderPanelLogic`, `applyMaxConcurrentPreference`, and
     `initJobControls`; panel logic should consume capabilities from the store
     owner and apply slices, not start separate lifecycle chains for the same
     event.
   - Add focused call-count tests around app settings hydration, encoder
     defaults, job controls startup, and explicit refresh.

5. Loose UI module cleanup on the touched path.
   - Move `src/ui/jobControls.ts` into `src/ui/jobControls/index.ts` if imports
     remain stable through `../jobControls` / `./jobControls`.
   - Add `src/ui/jobControls/AGENTS.md` if the move creates a real owned module
     surface worth documenting.
   - Do not sweep unrelated loose files such as `coverArt.ts`,
     `metadataForm.ts`, `metadataLookup.ts`, or `tagPreview.ts` unless the
     implementation touches them for the same ownership reason.

6. Trivial cleanup pull-forward.
   - Delete `src/effect-smoke.test.ts` after targeted frontend tests cover the
     real Effect workflow surfaces already in the repo.
   - Remove unused `normalizeEncoderAvailability` from
     `src/lib/tauri/normalizers.ts` if `rg` still finds no caller.
   - Leave output-artifact test-only export cleanup for WB-D.

7. Work Runtime lifecycle deletion gate.
   - Before calling the Work Runtime lifecycle spec done, finish or explicitly
     record the remaining repo facts from
     `docs/specs/work-runtime-lifecycle-retirement.md`: snapshot-only Work
     Center progress, Status Panel/preview/metadata-save classification,
     stale `docs/api-map.md` references, and linked issue #307 state.
   - Delete `docs/specs/work-runtime-lifecycle-retirement.md` only in the commit
     that satisfies that gate.

Verification steps:

- Focused encoder panel and encoding workbench Vitest coverage.
- Output panel tests for estimated-size display if touched.
- Runtime capability/job controls tests for call count, cache behavior, refresh
  behavior, and settled state.
- App Settings hydration tests for single ownership of capability loads.
- `bun scripts/check-tauri-runtime-boundary.ts`.
- `git diff --check`.

Expected repo-visible outcome:

- A branch that starts with WB-B, removes the dead encoder shell, fixes
  output-panel reach-through imports, gives runtime capability loading one
  owner, pulls forward safe trivial frontend/runtime cleanup, and either
  deletes the Work Runtime lifecycle spec after satisfying its deletion gate or
  leaves the spec visibly active with current repo evidence.

## Interfaces And Dependencies

- Frontend Public API Strips: encoder panel, output panel, app settings/job
  controls if capability ownership changes.
- Runtime boundary dependency: `tauriClient.getRuntimeSettingsCapabilities()`.
- Existing issue dependencies: #348 for capability ownership, #361 for roadmap
  tracking.

## Verification Path and Checks

Targeted checks:

- `bun run test -- src/ui/__tests__/encoderPanel-behavior.test.ts`
- `bun run test -- src/ui/__tests__/encodingWorkbench-island.test.ts`
- `bun run test -- src/ui/__tests__/outputPanel-store-driven.test.ts`
- `bun run test -- src/ui/__tests__/jobControls-info.test.ts`
- `bun run test -- src/ui/__tests__/runtimeSettingsCapabilities.test.ts`
- `bun run test -- src/ui/appSettings/appSettings.test.ts`
- Add/narrow tests based on the exact files touched.
- `bun scripts/check-tauri-runtime-boundary.ts`
- `git diff --check`

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring ownership rules into nearest frontend `AGENTS.md` files,
  `docs/system-map.md`, or `docs/ubiquitous-language.md`.
