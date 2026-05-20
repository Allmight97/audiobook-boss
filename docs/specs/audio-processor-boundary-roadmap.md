# Audio Engine Deep Module Roadmap

Status: active architecture and roadmap alignment
Owner surface: `src-tauri/src/audio/`, `src-tauri/src/audio/processor/`, `src-tauri/src/processing/`
Implementation posture: future branch from `main`; prefer one coherent PR unless discovery proves a split is safer.

This spec is temporary working state. Delete it after implementation, review,
validation, documentation alignment, merge, and sync are complete.

## Purpose / Big Picture

Audiobook Boss already has strong owned boundaries for metadata outcome,
processing plans, output artifact truth, and status rendering. Audio processing
is the next candidate for the same treatment because it is the place where media
execution, encoder/toolchain selection, stream integrity, cancellation,
metadata write timing, staging, finalization, and progress reporting currently
meet.

The roadmap goal is to make the Audio Engine Deep Module boundary durable
enough that future agents can modify codec, encoder, and media behavior without
relearning unrelated processing, metadata, output artifact, or status runtime
rules.

Good looks like:

- Audio/media execution has one clear owner and a small public edge.
- Processing orchestration can dispatch work without knowing media internals.
- Metadata and output artifact boundaries remain authoritative for their own
  truths.
- Status/progress is truthful without audio code becoming a status runtime.
- Behavior changes, if chosen, are explicit product decisions traced from UX
  outcome to code boundary.

## Scope And Constraints

In scope:

- Audit whether audio processing should become the Audio Engine Deep Module: a
  sixth Grey-Box Public API owner in repo-governed ABB terminology, or a named
  Private Cluster under an existing owner.
- Define the public edge for the Audio Engine Deep Module.
- Separate media execution truth from processing lifecycle truth.
- Audit native AAC, Apple AAC/AAC-AT, and external FDK execution paths for
  duplicated contracts, hidden adapters, and inconsistent finalization.
- Reconcile staging/finalization handoff with `output_artifact` ownership.
- Audit progress, cancellation, and terminal status residue left by recent
  Effect and Metadata Outcome work where it affects audio boundaries.

Non-goals:

- Do not implement this roadmap in this planning thread.
- Do not redesign metadata outcome ownership unless audio discovery proves a
  specific contract gap.
- Do not collapse processing, metadata, output artifact, and status panel
  runtime into a generic operation manager.
- Do not introduce fallback or compatibility behavior without explicit evidence,
  trigger, marker/register entry, and removal condition.
- Do not include remote acquisition work except as boundary contrast.

## Solution Posture

Chosen posture: subsystem boundary roadmap.

This is larger than an internal cleanup because `audio::processor` currently
touches media execution, external processes, staging, metadata writes, output
commit, progress, cancellation, and terminal success. A smaller local patch
would likely preserve the bad seam: media execution is not quite isolated from
processing lifecycle or final artifact truth.

The roadmap should still avoid a broad platform rewrite. The target is a clear
Audio Engine Deep Module boundary that cooperates with existing owners:

| Plain-language role | ABB term | General SWE terms |
| --- | --- | --- |
| Owned truth boundary | Grey-Box Public API owner | bounded context, subsystem, component boundary |
| Small allowed caller surface | Public API Strip | port, facade, service interface |
| Hidden implementation files | Private Cluster | adapter cluster, implementation module, internal package |
| Cross-boundary misuse | Reach-Through | layering violation, encapsulation break |
| Split product rule | Ownership Smear | leaky abstraction, misplaced responsibility |

Terminology rule for this roadmap: use **Audio Engine Deep Module** as the
human-facing name. When repo governance matters, translate that to "the audio
engine as a Grey-Box Public API owner with a Public API Strip and Private
Cluster."

## Context And Orientation

Current canonical owners:

| Owner | Current role | Audio roadmap impact |
| --- | --- | --- |
| Processing Plan | Preflight and execution planning before jobs run. | Should choose dispatch inputs without absorbing codec/media internals. |
| Metadata Outcome Plan | Effective metadata, naming metadata, write instructions, cover-art passthrough. | Audio execution should consume metadata decisions, not reconstruct them. |
| Output Artifact Plan / Commit | Requested/resolved paths, collision review, parent creation, final commit truth. | Audio finalization must delegate artifact commit truth here. |
| Status Panel Runtime | Backend progress/results rendered as truthful UI status and controls. | Audio should emit truthful stage signals without owning UI status policy. |
| Audio Engine Deep Module | Grey-Box Public API owner for media execution truth. | Owns decoder/toolchain/encoder/mux/staging media invariants behind `crate::audio`. |

Known current orientation:

- `src-tauri/src/audio/AGENTS.md` owns audio integrity rules across stream
  probing, decoder setup, resampling, sample buffering, encoder setup, muxing,
  and output validation.
- `src-tauri/src/audio/processor/AGENTS.md` owns processor stage orchestration,
  cancellation checkpoints, cleanup guarantees, and delegates final artifact
  decisions to `output_artifact`.
- `src-tauri/src/processing/AGENTS.md` owns preflight planning, execution-plan
  preparation, runner orchestration, processing context/session state, job
  lifecycle, progress event types, and terminal result normalization.
- `docs/system-map.md` lists Audio Engine Deep Module as the sixth Grey-Box
  Public API owner.
- `docs/ubiquitous-language.md` distinguishes Grey-Box Module, Public API Strip,
  Private Cluster, Deep Module, Reach-Through, Ownership Smear, Terminal Truth,
  and Backend Lifecycle.

## Plan Of Work

### M0 - Current Trace And Discovery

Trace current main from UX request to final artifact:

- selected files and processing settings
- preflight and execution plan
- processor adapter selection
- native/Apple/external FDK execution
- metadata write timing
- staging and final artifact commit
- progress, cancellation, skip, failure, and terminal result

Output: source-backed boundary map and list of verified seams.

### M1 - Audio Engine Deep Module Boundary Decision

Accepted boundary: audio processing becomes the sixth Grey-Box Public API owner
named Audio Engine Deep Module. Its public strip is `crate::audio`; its private
cluster includes processor internals, adapter routing, media execution,
staging, cleanup, and low-level toolchain details.

Output: encoded boundary decision and public strip.

### M2 - Runner Versus Engine Contract

Define what `processing` gives audio and what audio returns.

Candidate contract shape:

- processing owns queue, cancellation identity, run/session lifecycle, and
  terminal normalization
- the Audio Engine Deep Module owns media execution plan resolution, stream
  preparation, decode/resample/encode/mux, media staging, and media-integrity
  facts
- output artifact owns final commit truth
- metadata owns metadata outcome/write policy

Output: runner/engine interaction design and rejected alternatives.

Candidate Public API Strip shape, subject to scout validation:

- Inputs from processing: validated execution context/session, planned input
  files or media inputs, encoder/toolchain settings, metadata outcome/write
  facts, output/staging intent, cancellation/progress handles.
- Outputs to processing: staged artifact candidate or final media execution
  result, media-integrity facts, adapter/toolchain facts, and structured failure
  or cancellation facts.
- Private inside the Audio Engine Deep Module: adapter selection details,
  FFmpeg/native/FDK routing, frame pipeline internals, stream probing,
  sample-buffer mechanics, encoder helpers, external process choreography,
  temp media staging mechanics, and progress-message phrasing.
- Forbidden reach-through: processing, metadata, output artifact, or UI code
  importing private adapter/frame/finalize helpers to reconstruct media policy.

Candidate public-strip contract v0:

| Surface | Likely public | Likely private |
| --- | --- | --- |
| Source inspection | File-list/media inspection needed by commands, preflight, and UI. | Stream probing internals, decoder trial order, FFmpeg inspection structs. |
| Input validation | Audio input path validation and supported-input classification. | Low-level extension/container helpers unless another owner has an explicit need. |
| Encoder/toolchain availability | User/config-facing encoder and external toolchain capability facts. | Resolution strategy, fallback routing, external process argument construction. |
| Execution | One audio job execution entrypoint or request/result contract. | `FfmpegNextProcessor`, `MediaProcessor`, `MediaProcessingPlan`, `ResolvedProcessorAdapter`, adapter selection, frame pipeline, preview internals. |
| Finalization handoff | Return staged/final media execution facts needed by processing and output artifact. | Direct final path commit policy, replacement semantics, and output collision decisions. |

Provisional visibility default: the implementation agent may narrow Rust
visibility and module exports as part of this roadmap when source validation
shows the current `pub` surface is accidental. Public API changes must be
paired with call-site updates and boundary assertions.

### M3 - Adapter Convergence

Audit native AAC, Apple AAC/AAC-AT, and external FDK paths for duplicated or
divergent contracts.

Questions to answer:

- Which adapter differences are real media/toolchain differences?
- Which differences are accidental call choreography?
- Which helper APIs should be private to the audio engine?
- Which public result shape should every adapter return?

Output: adapter model and audio engine internal cluster plan.

### M4 - Finalization And Artifact Handoff

Audit staging, metadata rewrite, output commit, cleanup, and post-success proof.

The invariant to protect: audio can produce a candidate media artifact, but
`output_artifact` owns final artifact path and commit truth.

Output: finalized handoff model between audio engine, metadata outcome, and
output artifact boundaries.

### M5 - Operational Status Runtime Integration Decision

Make operational status runtime a formal design milestone, not a default
implementation milestone.

This milestone must audit whether recent Effect and Metadata Outcome changes
left status/progress or operation-lifecycle residue that affects audio
processing boundaries. It should then choose one of three outcomes:

- no status runtime change; audio only emits cleaner stage facts
- small status API/naming cleanup inside the existing Status Panel Runtime
- promote operational status runtime to a follow-on roadmap because the issue is
  broader than audio

Output: explicit integration decision with evidence.

### M6 - Implementation Handoff

Create the implementation-agent prompt only after M0-M5 are aligned.

The handoff should ask the implementation agent to validate the spec, create a
plan, work the plan as an active goal, and maintain an implementation notes file
for decisions, tradeoffs, changed assumptions, and follow-up items.

## Scout Lanes

Use targeted scouts during roadmap planning where assumptions are still soft:

- Audio source lane: trace `src-tauri/src/audio/` and `src-tauri/src/audio/processor/`.
- Processing lifecycle lane: trace runner, plan, terminal outcomes, queue, and
  cancellation surfaces.
- Output/finalization lane: trace staging, commit, cleanup, and resource
  lifetime risks.
- Status/progress lane: trace progress events, status panel runtime, metadata
  save status, and Effect workflow residue.
- Library/reference lane: use `abb-library-research` only where current
  FFmpeg/Tauri/Specta/Effect behavior changes the design decision.

Scout output should be synthesized by boundary, not dumped as separate reports.

## Scout Acceptance Criteria

The implementation-planning handoff is ready when source discovery can answer
these questions with file-backed evidence:

| Lane | Acceptance evidence |
| --- | --- |
| Public edge | Current audio exports are classified as keep-public, make-private, or needs-proof. The implementation agent has permission to narrow accidental visibility. |
| Runner/engine | Current `processing` calls into audio are classified as legitimate contract calls, temporary coupling, or reach-through. |
| Adapter convergence | Native, Apple/AAC-AT, and external FDK differences are separated into real toolchain differences versus duplicated choreography. |
| Finalization | Output artifact remains final commit truth; duplicated native/FDK staging, cleanup, cancellation, metadata rewrite, and success emit paths are explicitly in or out of M4. |
| Status runtime | Event/stage/queue contract is preserved by default. Audio-engine changes must preserve or explicitly improve terminal event emission, command-result reconciliation, and job-registry terminalization; any broader status change needs explicit evidence and smallest-scope status update. |
| Library/reference | `abb-library-research` is used only if FFmpeg/Tauri/Specta/Effect behavior affects the design decision. |

Handoff stop condition: if a lane finds a safety, data-loss, or IPC-contract
risk that cannot be localized to the Audio Engine Deep Module, pause
implementation planning and bring the decision back to roadmap alignment.

## M0 Discovery Snapshot

2026-05-20 source audit, read-only, no tests/checks run.

### Public Edge Findings

- `src-tauri/src/audio/mod.rs` currently publishes all audio submodules
  (`buffer`, `cleanup`, `constants`, `file_list`, `processor`, `settings`,
  `settings_encoder`, `toolchain`) plus re-exports for file info, validation,
  toolchain detection, cleanup, processor execution, and processor engine types.
- `src-tauri/src/audio/processor/mod.rs` currently publishes internal stage and
  adapter modules (`adapter`, `engine`, `execute`, `external_fdk`, `finalize`,
  `frame_pipeline`, `plan`, `prepare`, `selection`, `staging`, `streams`) and
  re-exports native-engine internals (`FfmpegNextProcessor`,
  `MediaProcessingPlan`, `MediaProcessor`, `PreviewAction`, `PreviewState`).
- Current public edge is therefore wider than the intended Audio Engine Deep
  Module strip. The roadmap should not assume the final strip is the existing
  export list; it should validate and narrow it.

### Processing Lifecycle Findings

- `processing/run.rs` owns lifecycle orchestration: validates settings,
  prepares execution plans, emits queue events, registers jobs, builds
  `ProcessingContext`, normalizes terminal outcomes, and reports command
  results.
- `processing/run.rs` reaches into audio for file info and adapter resolution
  before execution. That is legitimate today, but it is exactly the caller
  surface the Audio Engine Deep Module should simplify or explicitly preserve.
- A cleaner contract would let processing provide validated job/session/output
  context and receive an audio execution result, while audio privately decides
  adapter/toolchain details.

### Finalization And Artifact Findings

- Native audio finalization delegates final commit truth to `output_artifact`
  via `OutputCommitRequest`, `commit_output_artifact`, and
  `finalized_output_success`.
- External FDK performs its own staging, metadata rewrite, output commit,
  cleanup, cancellation check, and success emit instead of sharing the native
  finalize path.
- `output_artifact` already owns final path/collision/replacement commit truth;
  the roadmap should preserve that ownership and treat duplicated native/FDK
  finalization choreography as in-scope for M4.

### Status Runtime Findings

- Rust progress events use a small wire contract:
  `EventStage`, `ProgressEvent`, and `QueueEvent`.
- Audio currently emits stage facts through `ProcessingContext::new_emitter()`
  and `ProgressEmitter`. External FDK emits its own analyzing/converting/
  metadata/finalizing/cancelled/complete messages.
- Metadata save now reuses the processing queue/progress event contract and
  `ProcessResultSummary` shape, while the frontend `StatusPanelRuntime` tracks
  `currentWorkKind` values for `merge`, `batch`, and `metadataSave`.
- The status panel consumes terminal events and command results; it should not
  become the source of audio execution truth. The audio roadmap should instead
  preserve the event/result/lifecycle contract while simplifying where audio
  emits finalization, cancellation, cleanup, and completion facts.
- Current evidence supports keeping Operational Status Runtime as M5
  audit/decision scope, not default implementation scope. If the audio PR
  changes progress stages, event semantics, cancellation timing, or final
  success timing, M5 can expand to the smallest status-runtime cleanup needed.

### M0 Verdict

Implementation discovery promoted the owner hypothesis: Audio Engine Deep Module
is the sixth Grey-Box Public API owner.

Do not ask the implementation agent to redesign status runtime by default. Ask
it to track status/progress findings in implementation notes and expand M5 only
with source evidence.

## Accepted Alignment Defaults

These are the accepted defaults for implementation:

- Visibility: the implementation agent may narrow `pub` module exports and Rust
  visibility when doing so makes the Audio Engine Deep Module boundary more
  truthful. This is accepted scope, not churn, when it removes accidental public
  surface.
- Status runtime: preserve the existing progress/queue event contract unless
  audio correctness forces a change. Stage wording/timing may improve only when
  it makes terminal truth clearer; a new status model or event contract requires
  M5 evidence.
- Mandatory status acceptance rule: any refactor that moves native/FDK
  finalization must preserve or explicitly improve the sequence from backend
  terminal event emission to frontend command-result reconciliation to
  `JobRegistry` terminalization.
- PR shape: prefer one coherent PR for the roadmap. Split only for a concrete
  safety, reviewability, or contract-risk reason discovered during
  implementation planning.

## Pre-Handoff Alignment

Before further implementation work continues, keep these points aligned:

- Owner decision: Audio Engine Deep Module is the sixth Grey-Box Public API
  owner.
- Public strip framing: use `crate::audio` for callers; exact Rust symbols are
  enforced by `scripts/check-public-api-strips.sh`.
- Operational status runtime: keep it as a formal M5 audit/decision. Track
  status/progress/lifecycle surprises in the implementation notes scratch pad
  and expand implementation only when evidence shows audio cannot be made
  coherent without it.
- Scout acceptance: M0/M5 are related. The scout lanes are sufficient when they
  can say which current calls are legitimate boundary crossings, which are
  reach-through or ownership smear, and whether status runtime changes are
  necessary for truthful audio progress/cancel/finalization.

## Progress

- 2026-05-20: Created active roadmap spec after alignment that the Audio Engine
  Deep Module is the primary next roadmap target and Operational Status Runtime
  should be formally audited as an integration decision.
- 2026-05-20: Added M0 read-only discovery snapshot for current public edge,
  processing lifecycle, finalization/output handoff, and status-runtime scope.
- 2026-05-20: Added scout acceptance criteria, candidate public-strip contract
  v0, and provisional alignment defaults for visibility, status runtime, and PR
  shape.
- 2026-05-20: Accepted Rust visibility narrowing as roadmap scope and refined
  status-runtime scope to a mandatory event/result/lifecycle preservation rule,
  with broader status cleanup still evidence-gated.
- 2026-05-20: Implementation branch promoted Audio Engine Deep Module to a
  canon Grey-Box owner, encoded `crate::audio` as its Public API Strip, hid
  processor internals, replaced processing runner adapter reach-through with an
  audio-owned execution facade, and routed native/FDK artifact success through a
  shared finalization handoff.
- 2026-05-20: M5 status audit preserved existing IPC/event shapes and status
  runtime ownership; no generated binding, status model, or event-contract
  change was required.
- 2026-05-20: Manual xHE-AAC validation succeeded for Auto FDK preview output
  and explicit FDK full output using the local Star Trek fixture. Logs showed
  external FDK adapter selection, forced `aac_at` input decoding, cover-art
  embedding, delegated `output_artifact` hard-link commit, and `JobRegistry`
  completion for both outputs.
- 2026-05-20: Bulky private audio-engine tests were moved from inline
  `#[cfg(test)]` blocks into sibling source-tree `*_tests.rs` modules while
  preserving private-cluster visibility and keeping integration tests on the
  public strip.

## Surprises And Discoveries

- Source audit strengthened the owner hypothesis: the current audio public edge
  is wider than a clean Audio Engine Deep Module strip, so implementation
  planning should validate and narrow it rather than preserve exports by habit.
- External FDK duplicates finalization/output/status choreography that native
  processing mostly delegates through shared helpers; that convergence belongs
  in M4.
- Status runtime remains audit/decision scope. Current code shows shared
  progress/queue contracts and metadata-save reuse, but no evidence yet that a
  status-runtime redesign is required for the audio roadmap. The specific risk
  to guard is terminal lifecycle drift if audio finalization emission points
  move during native/FDK convergence.

## Accepted Decisions

- Use `docs/specs/audio-processor-boundary-roadmap.md` as the active planning
  surface for this roadmap.
- Treat the Audio Engine Deep Module as the sixth Grey-Box Public API owner.
- Use "Audio Engine Deep Module" as the human-facing roadmap label while
  preserving Grey-Box Public API owner / Public API Strip / Private Cluster as
  the repo-precise terminology.
- Allow the implementation agent to narrow accidental Rust public exports and
  module visibility when source validation supports the Audio Engine Deep Module
  boundary.
- Treat Operational Status Runtime as a formal milestone-level design audit and
  integration decision, not as automatic implementation scope. Lock bounded M5:
  expand status implementation only if terminal event emission, command-result
  reconciliation, or `JobRegistry` lifecycle truth cannot stay coherent
  otherwise.
- Preserve existing owner boundaries. The roadmap improves contracts between
  owners; it does not collapse them into one generic operation subsystem.
- Behavior mutations are allowed only when traced from user-visible outcome to
  boundary impact and explicitly chosen.
- Audio Engine Deep Module is canon once this implementation lands. Public
  callers use `crate::audio`; processor internals remain private cluster files.

## Validation And Acceptance

Planning acceptance:

- Current code and canon docs have been traced enough to remove stale or
  historical framing.
- The roadmap names the owner, public edge, private cluster responsibilities,
  and cross-boundary contracts.
- The roadmap includes a scout plan for assumptions and knowledge gaps.
- The implementation prompt is specific enough for a fresh agent to validate,
  plan, implement, record implementation notes, and prepare one coherent PR.

Implementation acceptance, to be refined later:

- Contract tests or equivalent boundary assertions cover the public strip.
- Audio correctness proof includes targeted tests and real media diagnostics
  where codec/media behavior changes.
- `scripts/checks.sh standard` runs before PR handoff if behavior/runtime code
  changes.
- Docs alignment is completed after implementation and before deleting this
  spec.

## Interfaces And Dependencies

Potentially affected surfaces:

- Rust public exports from `src-tauri/src/audio/mod.rs`
- Rust public exports from `src-tauri/src/audio/processor/mod.rs`
- `src-tauri/src/processing/run.rs`
- `src-tauri/src/processing/plan.rs`
- `src-tauri/src/processing/terminal_outcomes.rs`
- `src-tauri/src/output_artifact/`
- `src-tauri/src/metadata/`
- `src/ui/statusPanel/`
- generated IPC bindings only if public command/event shapes change
- `docs/system-map.md` and `docs/ubiquitous-language.md`

## Idempotence And Recovery

This spec is safe to revise during planning. If interrupted, resume by checking:

- current git branch and `main` sync state
- whether a newer implementation branch already exists
- the latest entries in Progress, Surprises And Discoveries, and Accepted
  Decisions

Do not create another spec for the same roadmap. Update this file.

## Completion And Cleanup

Before deleting this spec:

- implementation has merged and synced to `main`
- docs and nested `AGENTS.md` accurately reflect the final boundary
- stale roadmap/planning artifacts are purged
- final verification and PR review adjustments are complete

Delete the spec after completion; do not archive it as durable canon.
