# Module Cohesion Decomposition — Active Spec

Status: temporary active spec.
Tracker: GitHub issue #361, WB-D; GitHub issue #359.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: the largest backend/frontend pressure points are split along existing
private-cluster responsibilities so future remote acquisition, processing, and
artifact work does not keep adding branches to overgrown coordinators.

Acceptance signal: remote acquisition UI, Audible provider coordination,
RemoteSourceRuntime lifecycle, processing run orchestration, and dense pure
cores have clearer private modules or explicit threshold exceptions, with
behavior unchanged and owner-scoped tests proving the moved boundaries.

## Progress

- [ ] Refresh LOC/evidence from current `main` after PR #362 lands.
- [ ] Split remote acquisition UI workflow/controller from rendering.
- [ ] Deepen Audible per-title acquisition private modules.
- [ ] Extract RemoteSourceRuntime job/session lifecycle.
- [ ] Split processing run orchestration after Work Runtime lifecycle cleanup
  reduces the relevant paths.
- [ ] Clean up private-cluster/test-only false confidence surfaces while the
  same files are open.

## Surprises & Discoveries

- Observation: current largest pressure points are concentrated around remote
  acquisition and processing lifecycle.
  Evidence: issue #359 and current LOC checks.
- Observation: some pure core crates are semantically dense but defensible until
  new behavior expands them.
  Evidence: `crates/abb-metadata-core/src/lib.rs`,
  `crates/abb-output-artifact-core/src/lib.rs`.
- Observation: false-green and test-only surfaces live inside the same large
  provider/private-cluster areas.
  Evidence: issue #355, issue #304, `src-tauri/src/output_artifact/mod.rs`.

## Decision Log

- Decision: run this after the remaining Work Runtime lifecycle work unless a
  touched large module creates immediate risk.
  Rationale: lifecycle retirement should simplify `processing/run.rs` before
  decomposition.
  Date: 2026-06-07.
- Decision: split by ownership and behavior, not by LOC alone.
  Rationale: small files are not the goal; fewer correction loops and clearer
  test seams are.
  Date: 2026-06-07.
- Decision: include private-cluster/test-only cleanup while the relevant modules
  are open.
  Rationale: #304/#355 findings share the same owner surfaces as the module
  decomposition work.
  Date: 2026-06-07.

## Context And Orientation

- Primary issue owner: #359.
- Related cleanup issues: #304 private cluster/API/comment audit, #355 Audible
  false-green guard, #341 proof quality.
- Main backend paths: `src-tauri/src/remote_source/`,
  `src-tauri/src/remote_source/providers/audible/`,
  `src-tauri/src/processing/run.rs`, `src-tauri/src/output_artifact/`,
  `crates/abb-metadata-core/`, `crates/abb-output-artifact-core/`.
- Main frontend paths: `src/ui/remoteSource/`,
  `src/ui/statusPanel/controller.ts`,
  `src/ui/statusPanel/processingWorkflow.ts`.

## Scope And Constraints

In scope:

- Extract remote acquisition modal workflow/controller/state from
  `RemoteSourceAcquireDialog.svelte`.
- Keep remote acquisition component focused on auth/selection rendering and
  event binding.
- Split Audible provider coordinator into private modules for provider policy,
  acquisition orchestration, materialization handoff, Supplemental PDF policy,
  diagnostics/redaction, and cleanup where current code supports it.
- Extract RemoteSourceRuntime cancellation/session cleanup/purge behavior.
- Move companion artifact commit policy behind an owned helper if processing
  still embeds PDF-specific commit rules.
- Split `processing/run.rs` along dispatch, execution, terminal outcome, and
  artifact side-effect boundaries after Work Runtime lifecycle cleanup.
- Move dense pure-core internals into modules only when feature work would
  otherwise increase semantic density.
- Remove or productionize test-only false confidence paths from #355.
- Audit `output_artifact` public strip `#[allow(unused_imports)]` exports and
  keep only exports with a real public/test boundary reason.

Out of scope:

- Live Audible network tests.
- Provider-secret or raw-provider-payload exposure to UI/log surfaces.
- Behavior changes to acquisition, processing, metadata, or artifact commit
  unless required to preserve existing truth after moving code.
- Broad media execution proof without a focused design from #341.

Constraints:

- Preserve provider redaction and secret containment.
- Keep public IPC/generated bindings stable unless a contract change is
  explicitly justified and proved.
- Add `// EXCEPTION:` markers only when a large orchestrator is intentionally
  retained for a concrete external or ownership reason.
- Do not split pure core crates only to satisfy a line count.

## Plan Of Work

Edits:

- Refresh source-size and current-issue evidence.
- Pick the first owner slice based on dependency order and current branch risk.
- Extract private modules with no public API widening unless necessary.
- Move tests toward public/private boundary behavior, not helper existence.
- Delete stale comments and migration narration while touched, per #304.
- Update nearest `AGENTS.md` files for intentional strip changes.
- Update #359 and #361 with completed slices and any remaining triggers.

Verification steps:

- Remote source Rust tests for provider/runtime lifecycle slices.
- Focused Vitest coverage for remote acquisition UI workflow outcomes.
- Processing/output artifact tests for companion artifact commit behavior when
  touched.
- Core crate tests for pure module splits.
- Generated binding check only if IPC shapes change.
- `git diff --check`.

Expected repo-visible outcome:

- One or more large but owner-coherent PRs, each centered on a real private
  cluster and its proof route. Avoid tiny PRs that only move code without
  reducing future correction cost.

## Interfaces And Dependencies

- RemoteSourceRuntime public strip and provider-private modules.
- Processing run/public planning and terminal outcome helpers.
- Output Artifact Plan / Commit public strip.
- Status Panel Runtime and remote acquisition frontend modules.
- Related issues #359, #304, #355, #341.

## Verification Path and Checks

Targeted checks:

- `cargo nextest run -p abb-remote-source-core`
- `cargo nextest run -p audiobook-boss --lib remote_source`
- `cargo nextest run -p audiobook-boss --test all_tests <processing/output filters>`
- `bun run test -- src/ui/remoteSource/<focused tests>`
- `bun run test -- src/ui/statusPanel/<focused tests>` when touched
- Core crate package-selected tests for pure splits
- `git diff --check`

Manual evidence, if needed:

- Remote acquisition cancel/complete/purge paths preserve terminal truth and do
  not leak provider-private material.
- Processing sidecar/artifact behavior still matches final disk truth.

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring owner rules into nearest `AGENTS.md`,
  `docs/system-map.md`, `docs/ubiquitous-language.md`, or linked issues.
