# Audio Engine Private Cluster and Preview Truth - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose

Outcome: the Audio Engine Deep Module stays behaviorally stable while its
highest-risk private execution code becomes reviewable and preview artifact
truth is intentional.

Acceptance signal: external FDK encoding keeps its public entrypoint and output
behavior, but `external_fdk.rs` is decomposed by private mechanism. Adaptive
preview chapter marker handling is resolved: either preview artifacts really
emit markers and tests prove that, or the dead marker collection path is removed
and tests prove preview artifacts intentionally omit chapters.

## Current Evidence

- `src-tauri/src/audio/processor/external_fdk.rs` is the largest first-party
  source file and combines argument building, process spawn/monitor/kill,
  progress parsing, cancellation, temp staging, cleanup, metadata passthrough,
  finalization handoff, fake-FFmpeg fixtures, and tests.
- `src-tauri/src/audio/processor/AGENTS.md` already defines
  `src-tauri/src/audio/processor/` as the Audio Engine private execution
  cluster and allows private sibling test modules.
- `src-tauri/src/audio/processor/preview_state.rs` still carries
  `TODO(audio-preview)` for collected chapter markers.
- `src-tauri/src/audio/processor/frame_pipeline.rs` records adaptive preview
  chapter markers.
- `src-tauri/src/audio/processor/engine.rs` logs collected marker counts but
  does not emit preview chapters.
- `src-tauri/src/audio/processor/encoder/context.rs` skips chapter passthrough
  in preview mode.
- Existing external FDK tests already assert that preview suppresses passthrough
  chapters while full output keeps them.

## Decision Log

- Decision: combine external FDK private-cluster decomposition and preview
  marker truth into one Audio Engine workblock.
  Rationale: both are private Audio Engine concerns, and the FDK split creates
  the right fixture/test structure for proving native/external preview behavior
  without widening the public strip.
  Date: 2026-05-28.
- Decision: default preview-marker posture is delete dead marker collection
  while preserving current artifact behavior, unless implementation discovery
  finds an existing user-visible requirement that preview files contain
  chapters.
  Rationale: current code already skips preview chapter passthrough and only
  logs collected markers. Emitting new preview chapters would be a behavior
  change and needs explicit proof and product rationale.
  Date: 2026-05-28.

## Scope

In scope:

- Extract external FDK argument construction.
- Extract external process spawn/monitor/kill/wait behavior.
- Extract external progress parsing.
- Extract bulky fake-FFmpeg fixtures/helpers from inline tests where useful.
- Preserve external FDK staging, cleanup, cancellation, metadata passthrough,
  and finalization handoff behavior.
- Resolve preview chapter marker collection by removing it or by deliberately
  emitting preview markers through the correct metadata/media path.
- Add tests for args, progress, cancellation/error cleanup, and native/external
  preview chapter truth.

Out of scope:

- Processing terminal outcome classification.
- Output Artifact commit policy changes.
- Metadata Outcome Plan redesign.
- New encoder feature work.
- Full preview UX redesign.
- Full chapter authoring.

Constraints:

- Keep the Audio Engine public strip stable unless a narrow public-strip change
  is explicitly justified and tested.
- Do not bypass `output_artifact` finalization ownership.
- Preserve cleanup ownership transitions; remove cleanup ownership only after
  durable artifact commit.
- Kill and wait external processes deterministically on cancellation/error.
- Do not imply preview chapters exist in UI/status unless artifacts actually
  contain them.
- Keep behavior consistent across native and external FDK preview paths.

## Plan Of Work

- Split `external_fdk.rs` into private modules under
  `src-tauri/src/audio/processor/` for args, process lifecycle/progress,
  progress parsing, and test fixtures as needed.
- Move code mechanically first, then make local readability improvements only
  where they delete scan cost or duplicated setup.
- Resolve `TODO(audio-preview)`:
  - preferred route: remove marker collection and related tests if preview
    chapters remain out of scope;
  - alternate route: emit markers through the owning metadata/media path and
    prove actual preview artifact chapters.
- Keep final artifact commit policy delegated to `output_artifact`.
- Keep private-cluster invariant tests near the owning module without widening
  visibility.

## Proof Path

- Focused Rust audio processor tests for external FDK args, progress parsing,
  cancellation/error cleanup, and preview mode.
- Fixture/probe verification only if the implementation writes preview chapter
  metadata.
- `scripts/check-public-api-strips.sh`.
- `scripts/check-no-bridge-imports.sh`.
- `bun scripts/proof/runner.ts review` before handoff.

## Cleanup Trigger

When implemented, reviewed, validated, docs-aligned, and synced:

- Delete this spec.
- Distill any enduring Audio Engine private-cluster or preview-artifact rule
  into `src-tauri/src/audio/processor/AGENTS.md` only if future agents need it.
