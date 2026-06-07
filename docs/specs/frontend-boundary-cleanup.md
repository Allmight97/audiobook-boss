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

## Decision Log

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

Edits:

- Confirm runtime import tree for the canonical encoder workbench component.
- Remove old encoder island and orphan tests.
- Replace private output-panel state imports with Public API Strip access.
- Decide and implement runtime capability load/cache/apply ownership.
- Move or justify loose UI module files that shadow folders when touched.
- Update `src/ui/encoderPanel/AGENTS.md`, `src/ui/outputPanel/AGENTS.md`, and
  related tests if public strips change.

Verification steps:

- Focused encoder panel and encoding workbench Vitest coverage.
- Output panel tests for estimated-size display if touched.
- Runtime capability/job controls tests for call count and settled state.
- `bun scripts/check-tauri-runtime-boundary.ts`.
- Public API Strip checks if command exists for the touched surface.
- `git diff --check`.

Expected repo-visible outcome:

- One PR that removes the dead encoder shell, fixes the reach-through import,
  and leaves runtime capability ownership explicit enough for future settings
  work.

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
- Add/narrow tests based on the exact files touched.
- `bun scripts/check-tauri-runtime-boundary.ts`
- `git diff --check`

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring ownership rules into nearest frontend `AGENTS.md` files,
  `docs/system-map.md`, or `docs/ubiquitous-language.md`.
