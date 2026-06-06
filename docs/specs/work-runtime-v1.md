# Work Runtime V1 — Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: Audiobook Boss can accept long-running work as immutable background
operations, return the FileList to drafting immediately after backend
acceptance, and show multiple batch/merge operations in a Work Center.

Acceptance signal: the user can submit a batch encode, keep using FileList,
compose and submit a merge while the batch is still running, cancel either
operation independently, and verify terminal summaries against disk truth.

## Progress

- [x] 2026-06-06: Pro consultation synthesized; owner accepted FileList as
  Draft Workspace, functional V1, and module-size guardrails.
- [x] 2026-06-06: Add WorkRuntime pure state and runtime proof.
- [x] 2026-06-06: Wire batch/merge processing through WorkRuntime.
- [x] 2026-06-06: Add Work Center and draft-unlock workflow.
- [x] 2026-06-06: Evaluate acquisition + Source Inbox in this PR; defer V1B
  wiring because V1A already touches the processing runtime, status workflow,
  IPC contract, and new Work Center UI, and acquisition needs its own remote
  runtime/inbox UX slice to avoid leaking provider concerns or bloating the
  existing modal.

## Surprises & Discoveries

- Observation: current processing and remote acquisition each own separate job
  state, cancellation, and progress surfaces.
  Evidence: `src-tauri/src/processing/job_registry/`,
  `src-tauri/src/remote_source/mod.rs`, `src/ui/statusPanel/`,
  `src/ui/remoteSource/RemoteSourceAcquireDialog.svelte`.
- Observation: major likely touch points are already large.
  Evidence: `remote_source/mod.rs` ~779 LOC, `processing/run.rs` ~837 LOC,
  `RemoteSourceAcquireDialog.svelte` ~755 LOC, `statusPanel/controller.ts`
  ~476 LOC at spec creation.

## Decision Log

- Decision: FileList is a Draft Workspace, not the queue.
  Rationale: live FileList mutation must not affect accepted work; this is the
  unlock that allows batch and merge workflows to coexist.
  Date: 2026-06-06.
- Decision: V1 is functional, not scaffolding-only.
  Rationale: the owner needs a pressure-testable workflow before returning to
  power use.
  Date: 2026-06-06.
- Decision: WorkRuntime owns operation truth; RemoteSourceRuntime keeps provider
  truth.
  Rationale: work orchestration needs one event/cancel/status model without
  leaking provider secrets or Audible internals.
  Date: 2026-06-06.
- Decision: Source Inbox is the default remote-acquisition handoff model.
  Rationale: acquisition completion should produce retained usable material,
  not immediately mutate the current draft or purge useful staged files when the
  draft is busy.
  Date: 2026-06-06.

## Context And Orientation

- Current repo state checked on branch `feat/work-runtime-v1`.
- Canon terms that matter:
  - Runtime Boundary: frontend IPC routes through `tauriClient`.
  - Backend Lifecycle: current processing sub-owner for operation identity,
    queue/progress, cancellation, and terminal truth.
  - RemoteSourceRuntime: provider-neutral remote acquisition surface; provider
    secrets and raw payloads remain backend-private.
  - Terminal Truth / Artifact Truth: Rust reports durable final state.
- Canon surfaces this spec must not redefine:
  - `docs/system-map.md`
  - `docs/ubiquitous-language.md`
  - `docs/api-map.md`
  - nearest nested `AGENTS.md` files.

## Scope And Constraints

In scope:

- Background processing operations for `processingBatch` and `processingMerge`.
- Immutable work submissions accepted by backend before execution completes.
- Work Center UI read model for multiple operations.
- FileList draft unlock after accepted submit.
- Per-operation cancellation.
- Resource-lane vocabulary sufficient for encode CPU, output commit, and future
  acquisition lanes.
- Remote acquisition + Acquired Sources inbox if V1A stays stable enough.

Out of scope:

- Persistent operation history across app restart.
- Pause/resume and priority editing.
- Multi-instance orchestration.
- External queue/service platforms.
- Metadata save as a required first slice, though the operation model should not
  prevent it.

Constraints:

- Do not make already-large modules larger except for narrow routing glue.
- New backend work belongs in `src-tauri/src/work_runtime/`.
- New frontend work belongs in `src/ui/workCenter/`.
- Add local `AGENTS.md` files for new ownership surfaces.
- `processing/run.rs` must not become WorkRuntime.
- `remote_source/mod.rs` must not absorb WorkRuntime state or Work Center
  concerns.
- `RemoteSourceAcquireDialog.svelte` should shrink toward auth/selection UI if
  acquisition is wired.
- No reach-through imports into private clusters.
- No silent fallback/shim behavior.

## Plan Of Work

- Edits:
  - Create WorkRuntime public strip, runtime state, event types, commands, and
    Tauri registration.
  - Add pure operation-state tests and fake-executor runtime tests before real
    processing wiring.
  - Add processing adapter that submits immutable batch/merge work and drives
    existing processing execution behind WorkRuntime.
  - Add Work Center UI and update Status Panel / FileList integration.
  - Add Source Inbox only after V1A is functional and reviewed locally.
- Verification steps:
  - Run focused Rust tests for new core/runtime state.
  - Regenerate/check generated bindings.
  - Run focused Tauri client and Work Center Vitest coverage.
  - Run manual app smoke for batch + merge coexistence.
  - Run GPT-5.5 xhigh read-only child-thread review before push.
- Expected repo-visible outcome:
  - A PR branch with functional V1A and either V1B acquisition/inbox or an
    explicit active-spec remaining-work section.

## Interfaces And Dependencies

- New Rust owner: `src-tauri/src/work_runtime/`.
- New frontend owner: `src/ui/workCenter/`.
- Commands:
  - submit processing operation.
  - list/get work operations.
  - cancel work operation.
- Events:
  - operation snapshot.
  - operation list snapshot.
  - resource snapshot if useful for UI.
- Types:
  - OperationId.
  - OperationKind: `processingBatch`, `processingMerge`, `remoteAcquisition`,
    `metadataSave`.
  - OperationSnapshot.
  - ChildJobSnapshot.
  - ProgressSnapshot.
  - OperationTerminalSummary.
  - ResourceLane.

## Verification Path and Checks

Targeted checks:

- `cargo nextest run -p abb-processing-core`
- targeted `cargo nextest run -p audiobook-boss --lib <work-runtime tests>` as
  available.
- `bash scripts/check-generated-bindings.sh --mode local`
- `bun scripts/check-tauri-runtime-boundary.ts`
- `bun run test -- src/lib/tauri-public-api.contract.test.ts src/lib/tauri-client.test.ts src/lib/tauri-client.generated-event-bindings.test.ts`
- focused Work Center / Status Panel / FileList Vitest selections.

Manual evidence:

- Batch operation accepted and FileList usable immediately afterward.
- Merge operation accepted while batch operation still runs.
- Work Center shows independent operation progress and terminal summaries.
- Operation cancel does not cancel unrelated operation.
- Output files and terminal summaries match disk truth.
- Acquisition + Source Inbox scenarios if V1B lands.

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring behavior into the smallest canon surfaces:
  - `docs/system-map.md`
  - `docs/ubiquitous-language.md`
  - `docs/api-map.md`
  - nearest nested `AGENTS.md`
  - GitHub issue for explicit deferred V1B items, if needed.
