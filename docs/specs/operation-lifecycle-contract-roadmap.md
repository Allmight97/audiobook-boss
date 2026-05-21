# Operation Lifecycle Contract Roadmap

Status: active architecture and roadmap alignment
Owner surface: `src-tauri/src/processing/`, `src-tauri/src/commands/metadata/save_batch.rs`, `src/ui/statusPanel/`
Implementation posture: future branch from `main`; prefer one coherent PR unless discovery proves a split is safer.
Accepted owner posture: Backend Lifecycle becomes an explicit sub-owner/public
strip inside `processing`, not a new grey-box owner yet.
Accepted lifecycle vocabulary posture: introduce bounded operation vocabulary
inside the lifecycle strip when it removes call choreography and improves
agent-readable terminal truth.

Repo-level roadmap anchor: clean, agent-first codebase/repo, including testing
and proof infrastructure. This roadmap should improve ABB's ability to be
understood, changed, and verified by future agents without transcript
archaeology or implementation spelunking.

This spec is temporary working state. Delete it after implementation, review,
validation, documentation alignment, merge, and sync are complete.

## Purpose / Big Picture

Roadmap mission statement:

> Make ABB's operation lifecycle contract explicit so processing, metadata save,
> audio engine, and future long-running work can report truthful progress,
> cancellation, and final outcomes without each feature inventing status
> choreography.

This work matters now because recent Effect, Metadata Outcome, and Audio Engine
Deep Module roadmaps clarified their owners. The remaining cross-cutting seam is
not "the status UI" by itself; it is the lifecycle contract between backend
operation owners and the frontend read model that renders operation truth.

Good looks like:

- Backend lifecycle owns job identity, queue snapshots, cancellation,
  terminalization, and result summaries in a reusable way.
- Status Panel Runtime remains the UI read-model and interaction owner, not a
  backend operation manager.
- Processing and metadata save use one coherent lifecycle vocabulary without
  accidental coupling.
- Audio Engine Deep Module emits media-stage facts without owning UI status
  policy.
- Future long-running work can copy the operation pattern deliberately instead
  of inventing another status path.
- Future agents can find the lifecycle owner, public strip, proof path, and
  terminal truth vocabulary from the repo itself.
- Long-running operations carry explicit operation identity and terminal-summary
  vocabulary when that keeps status rendering and command results from relying
  on caller choreography.

## Scope And Constraints

In scope:

- Define the current operation lifecycle contract across processing, metadata
  save, progress events, queue events, cancellation, terminal results, and status
  rendering.
- Decide whether Backend Lifecycle needs a clearer public strip around
  `JobRegistry`, progress/queue events, operation kinds, and terminal outcomes.
- Decide whether event names and progress percentages currently in
  `src-tauri/src/audio/constants.rs` should move to a processing/status-owned
  home.
- Audit whether metadata save is sharing lifecycle machinery by design or by
  convenience.
- Audit whether Status Panel Runtime's `currentWorkKind` and event reducers are
  the right read-model shape for multiple operation families.
- Decide whether `src-tauri/src/processing/run.rs` should remain the processing
  runner/orchestrator while delegating generic lifecycle mechanics.
- Decide how to split `src-tauri/src/audio/constants.rs` so lifecycle/status
  constants stop living in audio-owned space.

Non-goals:

- Do not redesign the UI.
- Do not implement remote acquisition.
- Do not combine this roadmap with testing/proof infrastructure; defer that to
  issue #323.
- Do not collapse Processing Plan, Metadata Outcome Plan, Audio Engine Deep
  Module, Output Artifact Plan / Commit, and Status Panel Runtime into one
  generic operation blob.
- Do not introduce compatibility fallback behavior without explicit evidence,
  trigger, marker/register entry, and removal condition.
- Do not introduce a generic operation framework that hides the specific owners
  for processing, metadata, audio, output artifacts, or status rendering.

## Solution Posture

Chosen posture: subsystem contract roadmap.

This is broader than moving constants because the current seam spans backend
lifecycle, emitted event contracts, command result shapes, metadata-save reuse,
and frontend status derivation. A narrow patch might fix the most obvious
misplaced constants while preserving the deeper ambiguity: which owner defines a
truthful operation lifecycle?

This is not a platform rewrite. The target is an explicit contract and small
boundary cleanup that helps future agents reason about long-running work.

## Context And Orientation

Current canonical owners:

| Owner | Current role | Roadmap impact |
| --- | --- | --- |
| Processing Plan | Preflight and execution planning before processing jobs run. | Should not become a generic operation framework by accident. |
| Backend Lifecycle | Rust sequence that plans, queues, runs, cancels, skips, fails, succeeds, and finalizes processing work. | Candidate home for operation lifecycle contract vocabulary. |
| Status Panel Runtime | Renderable status truth, controls, progress/queue read-model, and cancellation UI. | Should consume lifecycle facts and expose UI actions without owning backend truth. |
| Metadata Outcome Plan | Metadata write policy and outcome planning. | Metadata save needs lifecycle reporting without smearing metadata policy into status runtime. |
| Audio Engine Deep Module | Media execution truth. | Should emit audio stage facts through the lifecycle contract without owning progress policy. |
| Output Artifact Plan / Commit | Final artifact path and commit truth. | Terminal success must remain downstream of artifact truth where processing outputs are involved. |

Current evidence anchors:

- `docs/ubiquitous-language.md` already defines **Backend Lifecycle** and
  **Operational Truthfulness**.
- `src-tauri/src/processing/job_registry/` owns concurrency, cancellation, and
  active-job state for processing work.
- `src-tauri/src/processing/progress/` owns `EventStage`, `ProgressEvent`, and
  `QueueEvent`.
- `src-tauri/src/processing/types.rs` owns `ProcessResultSummary` and command
  result shapes used by processing and metadata save.
- `src-tauri/src/commands/metadata/save_batch.rs` reuses `JobRegistry`,
  `QueueEvent`, `ProgressEvent`, and `ProcessResultSummary`.
- `src/ui/statusPanel/` owns event subscription, progress/queue reduction,
  cancellation UX, terminal feedback, and render state.
- `src-tauri/src/audio/constants.rs` still holds progress event names and
  progress math constants consumed by processing/status code.

## M0 Source Findings

### `src-tauri/src/processing/run.rs`

Current role:

- Entry point for executing and preflighting `ProcessPayload`.
- Validates encoder settings and the audio-engine input contract before run.
- Dispatches merge versus batch processing from the resolved processing plan.
- Emits batch queue snapshots.
- Schedules batch jobs through `JobRegistry`'s scheduler.
- Registers jobs, validates output paths, builds `ProcessingContext`, and passes
  execution to `crate::audio::execute_audio_engine`.
- Maps audio/processing outcomes into terminal `ProcessResultEntry` values and
  updates `JobRegistry` completion/failure state.

Design reading:

- It is no longer the old media/metadata monolith; audio execution and metadata
  outcome planning have owners.
- It is still the place where backend lifecycle semantics are most visible:
  queue emission, registration, cancellation checker creation, terminal result
  mapping, skipped/failed event emission, and batch result normalization.
- The roadmap should treat `run.rs` as the current processing-run adapter and
  exemplar of lifecycle responsibilities, not as canon for where every lifecycle
  rule must stay.

Target posture:

- Keep processing-specific orchestration in the runner: merge/batch dispatch,
  output-plan interaction, and audio execution request assembly.
- Move or facade generic lifecycle mechanics when they are shared with metadata
  save or future long-running operations.
- Avoid extracting a vague "operation framework" that erases processing,
  metadata, output artifact, and audio ownership.

### `src-tauri/src/audio/constants.rs`

Current role:

- Holds processing event names: `processing-progress` and `processing-queue`.
- Holds lifecycle progress percentages and progress math constants.
- Holds audio/media constants such as the temporary merged filename and allowed
  cover-art image extensions.

Design reading:

- This is a mixed-ownership file. Event names, progress bands, ETA formatting,
  and lifecycle percentage math are not audio-engine concepts.
- The audio `AGENTS.md` already marks these constants as temporary crate-visible
  compatibility until they move to a processing/status owner.
- Audio-specific constants can stay under audio, but lifecycle/status constants
  should move beside the progress/event contract that owns them.

Target posture:

- Move event names and lifecycle progress math to `processing/progress` or the
  accepted Backend Lifecycle public strip.
- Keep audio/media facts under audio, or split them into narrower audio-owned
  homes if `constants.rs` stops carrying enough coherent responsibility to
  justify existing.

### `src-tauri/src/commands/metadata/save_batch.rs`

Current role:

- Uses `JobRegistry`, `CancellationChecker`, `QueueEvent`, `ProgressEvent`, and
  `ProcessResultSummary`.
- Emits queue/progress events by hand through audio-owned event-name constants.
- Keeps metadata policy in metadata-owned APIs through `plan_metadata_write` and
  `save_metadata_with_plan`.

Design reading:

- Metadata save is the strongest proof that lifecycle primitives are already
  cross-operation. It shares the ingredients, but not through a named lifecycle
  contract.
- The roadmap should align metadata-save lifecycle reporting without moving
  metadata policy out of metadata.

### `src-tauri/src/processing/progress/`

Current role:

- Owns `EventStage`, `ProgressEvent`, `QueueEvent`, `QueueItem`, the
  `tauri_specta::Event` names, progress emission helpers, and progress math
  helpers.
- Still imports lifecycle constants from audio.

Design reading:

- This is the most natural existing home for progress/queue event ownership.
- If Backend Lifecycle becomes an explicit public strip, this directory is the
  likely source of its event vocabulary rather than a subordinate helper.

## Candidate Outcomes To Consider

### Outcome A - Lifecycle Contract Clarification

Define the operation lifecycle terms and public strip:

- operation kind
- job id and optional input index
- queued, active, terminal, and aggregate states
- progress stage and progress percentage
- cancellation requested versus terminal cancelled
- command result summary versus progress event terminal state

This is the minimum useful roadmap outcome.

Accepted direction:

- Introduce bounded lifecycle vocabulary inside `processing`, including an
  operation identity concept if implementation validation confirms it removes
  status/caller choreography.
- Prefer clear names such as `OperationKind` and shared terminal-summary
  vocabulary over continuing to reuse processing-specific names for metadata
  save.
- Accept generated binding churn when it buys contract clarity; reject churn
  that only renames shapes without reducing ambiguity.

### Outcome B - Backend Lifecycle Public Strip

Create or clarify the allowed import surface for lifecycle primitives:

- `JobRegistry`
- `CancellationChecker`
- `ProgressEvent`
- `QueueEvent`
- `EventStage`
- `ProcessResultSummary`
- terminal outcome helpers or operation result builders, if they deserve to be
  public within the backend
- event names, if direct Tauri emission remains necessary outside
  `ProgressEmitter`

The purpose is not to make a big framework; it is to prevent every long-running
command from hand-rolling lifecycle shape.

Accepted direction:

- Backend Lifecycle should become a named public strip/sub-owner inside
  `processing`.
- Do not promote it to a seventh grey-box owner unless implementation discovery
  proves `processing` can no longer coherently own the lifecycle contract.
- Implementation may extract shared lifecycle helpers from `run.rs` and
  `metadata/save_batch.rs` when that removes duplicated lifecycle choreography
  without blurring processing, metadata, audio, output-artifact, or status
  ownership.

### Outcome C - Constants Ownership Cleanup

Move event names and progress percentages out of audio-owned constants if
discovery confirms they are lifecycle/status concepts, not audio concepts.

This likely means audio keeps media facts while processing/status owns event
names, stage percentages, and wire lifecycle math.

Design-away option:

- Delete the concept of one broad `audio/constants.rs` if it no longer has a
  coherent owner. Replace it with lifecycle constants beside progress events and
  focused audio constants beside the code that needs media facts.

### Outcome D - Metadata Save Lifecycle Alignment

Decide whether metadata save should:

- keep using `JobRegistry` and processing progress event shapes directly,
- use a small lifecycle facade shared with processing, or
- get a narrower metadata-save-specific adapter that maps into the common
  lifecycle contract.

The target is truthful lifecycle reporting without moving metadata policy.

### Outcome E - Status Runtime Read-Model Boundary

Audit whether Status Panel Runtime's current reducer model is the right boundary
for multiple operation families.

Possible decisions:

- no change; status already consumes generic-enough lifecycle facts
- small naming/API cleanup inside Status Panel Runtime
- follow-on frontend roadmap if status complexity is mostly UI-layer glue

## Plan Of Work

### M0 - Current Trace And Discovery

Trace the current lifecycle path for:

- processing merge
- processing batch
- metadata save batch
- cancellation all
- cancellation single job
- skipped output
- failed job
- successful output after artifact commit

Also classify `run.rs` responsibilities into processing-specific orchestration
versus reusable lifecycle mechanics, and classify `audio/constants.rs` constants
into lifecycle/status versus audio/media ownership.

Output: source-backed lifecycle map and list of contract seams.

### M1 - Ownership Decision

Decide whether the lifecycle contract belongs under existing `processing/`,
becomes a named Backend Lifecycle public strip, or remains split with explicit
reasons.

Output: accepted owner model and rejected alternatives.

Decision: Backend Lifecycle belongs under `processing/` as an explicit
sub-owner/public strip for this roadmap. Rejected for now: a new standalone
grey-box owner, because the current lifecycle machinery is already rooted in
processing and can be made agent-readable before adding another top-level owner.

### M2 - Public Strip Shape

Name the lifecycle primitives that callers may use and the internals they may
not reach through.

Output: candidate public strip and private cluster responsibilities.

Decision: the public strip should allow callers to use lifecycle primitives for
job identity, cancellation, queue snapshots, progress events, operation
identity, and terminal-summary vocabulary. It should not expose runner
internals, processing planner internals, metadata policy, audio execution
internals, or status-panel reducer internals.

### M3 - Metadata Save Alignment

Audit and design how metadata save should report lifecycle truth.

Output: metadata-save lifecycle decision with explicit non-movement of metadata
policy.

### M4 - Status Runtime Read-Model Decision

Audit whether status runtime needs no change, small cleanup, or a follow-on UI
roadmap.

Output: bounded status runtime decision.

Decision: Status Panel Runtime remains a consumer/read-model owner. It may
consume lifecycle operation identity if the backend event/result contract grows
that fact, but this roadmap should not redesign the UI or make Status Panel the
backend operation owner.

### M5 - Constants And Event Ownership Cleanup Plan

Decide whether progress constants/event names should move, and what proof is
needed if they do.

Output: source-backed ownership choice and migration constraints.

Decision: implementation is authorized to move lifecycle event names, progress
bands, and progress math out of `audio/constants.rs` into the accepted lifecycle
or progress owner. Audio-specific constants may remain under audio or move to
smaller audio-owned homes if the broad constants module stops being coherent.

### M6 - Implementation Handoff

Create the implementation-agent prompt only after M0-M5 are aligned.

Output: handoff prompt asking the implementation agent to validate the spec,
plan implementation, work it as an active goal, and keep implementation notes.

Handoff readiness: ready. M0-M5 have accepted defaults sufficient for a tactical
implementation agent to validate, plan, and execute without reopening the
roadmap. Remaining choices should be handled as implementation validation unless
they would violate an accepted boundary or expand scope.

## Progress

- 2026-05-20: Created active roadmap spec after Audio Engine Deep Module landed
  and after deferring testing/proof infrastructure to issue #323.
- 2026-05-20: Locked mission statement: make ABB's operation lifecycle contract
  explicit so processing, metadata save, audio engine, and future long-running
  work can report truthful progress/cancel/final outcomes without each feature
  inventing status choreography.
- 2026-05-20: Deleted completed Audio Engine Deep Module roadmap spec artifacts
  from `docs/specs/`.
- 2026-05-20: Added repo-level roadmap anchor: clean, agent-first codebase/repo,
  including testing/proof infrastructure.
- 2026-05-20: M0 source read identified `run.rs` as the current processing-run
  adapter/exemplar for lifecycle responsibilities, `audio/constants.rs` as a
  mixed-ownership cleanup target, and `metadata/save_batch.rs` as proof that
  lifecycle primitives are already cross-operation.
- 2026-05-20: Accepted owner posture: Backend Lifecycle becomes an explicit
  sub-owner/public strip inside `processing`, not a new grey-box owner yet.
- 2026-05-20: Accepted implementation scope: the roadmap may move lifecycle
  constants out of `audio/constants.rs` and extract shared lifecycle helpers
  from `run.rs` / `metadata/save_batch.rs` when that reduces call choreography.
- 2026-05-20: Created temporary companion visual
  `docs/specs/operation-lifecycle-contract-roadmap.html` to explain the
  before/after state and the operation-vocabulary tradeoff before handoff.
- 2026-05-20: Accepted bounded lifecycle vocabulary: introduce operation
  identity and shared terminal-summary vocabulary inside the processing-owned
  lifecycle strip when it reduces call choreography and improves agent
  readability.
- 2026-05-20: Marked roadmap handoff-ready. Remaining tactical decisions should
  be validated by the implementation agent against current code and escalated
  only if they violate accepted scope, ownership, or behavior boundaries.

## Surprises And Discoveries

- The status/runtime seam is better framed as an operation lifecycle contract
  issue than as a frontend status-panel issue.
- `audio/constants.rs` still contains progress event names and percentages even
  though Audio Engine Deep Module is now a media-execution owner.
- Existing remote-acquisition planning already warns that sharing `JobRegistry`
  blindly can create an ownership smear. This roadmap should solve the lifecycle
  vocabulary before future long-running feature work.
- `processing/progress` already owns the event payload types and Specta event
  names, which makes audio-owned lifecycle event constants look accidental.

## Accepted Decisions

- Current roadmap target: Operation Lifecycle Contract / Backend Lifecycle
  Contract.
- Testing/proof infrastructure is deferred to issue #323 and should not be
  bundled into this roadmap.
- Remote acquisition is out of scope.
- UI redesign is out of scope.
- The roadmap starts by deciding ownership and contract shape, not by refactoring
  Status Panel Runtime first.
- Repo-level anchor for this and later roadmaps: make ABB cleaner and more
  agent-first, including proof infrastructure, without stuffing all cleanup into
  this single roadmap.
- Backend Lifecycle becomes an explicit sub-owner/public strip inside
  `processing`, not a standalone seventh grey-box owner for this roadmap.
- Introduce bounded lifecycle vocabulary inside the processing-owned lifecycle
  strip, including operation identity if implementation validation confirms the
  benefit.
- Implementation may move lifecycle constants out of `audio/constants.rs`.
- Implementation may extract shared lifecycle helpers from `run.rs` and
  `metadata/save_batch.rs` if the extraction improves ownership clarity and does
  not create a generic operation blob.
- Status Panel Runtime remains a consumer/read model. It may consume lifecycle
  operation identity, but it does not become the backend lifecycle owner.

## Validation And Acceptance

Planning acceptance:

- Current code and canon docs have been traced enough to explain operation
  lifecycle ownership without stale audio-roadmap framing.
- The roadmap names lifecycle owner options, public strip candidates, and
  non-goals.
- The implementation handoff is specific enough for a fresh agent to validate,
  plan, implement, record implementation notes, and prepare one coherent PR.
- No unresolved alignment question remains that must be answered before an
  implementation agent can produce its own tactical implementation plan.

Implementation acceptance, to be refined later:

- `scripts/check-public-api-strips.sh` reflects any accepted lifecycle public
  strip changes.
- `scripts/check-no-bridge-imports.sh` still prevents private-boundary reach
  through.
- `bash scripts/check-context-surface.sh` runs after docs/guidance updates.
- `scripts/checks.sh standard` runs before PR handoff if runtime behavior,
  events, cancellation, or generated contract behavior changes.
- User-visible progress, cancel, skip, fail, and success behavior remains
  truthful.
- If operation identity or lifecycle result vocabulary changes the generated
  contract, regenerated bindings and frontend consumers remain contract-aligned.

## Interfaces And Dependencies

Potentially affected surfaces:

- `src-tauri/src/processing/job_registry/`
- `src-tauri/src/processing/progress/`
- `src-tauri/src/processing/types.rs`
- `src-tauri/src/processing/run.rs`
- `src-tauri/src/commands/metadata/save_batch.rs`
- `src-tauri/src/audio/constants.rs`
- `src/ui/statusPanel/`
- `src/types/events.ts`
- `src/lib/generated/tauri.ts` only if event shapes change
- `docs/system-map.md`
- `docs/ubiquitous-language.md`
- nested `AGENTS.md` files under affected owners

## Idempotence And Recovery

This spec is safe to revise during planning. If interrupted, resume by checking:

- current git branch and `main` sync state
- whether a newer implementation branch already exists
- latest Progress, Surprises And Discoveries, and Accepted Decisions entries
- whether issue #323 changed testing/proof infrastructure scope

Do not create another spec for this same roadmap. Update this file.

## Completion And Cleanup

Before deleting this spec:

- implementation PR is merged and synced
- review-agent feedback is resolved or explicitly deferred
- canon docs and nested `AGENTS.md` files match the landed architecture
- no temporary implementation notes remain in the repo
- any deferred work is captured in a GitHub issue, not this spec

Delete this spec after completion. Do not archive it.
