# Adaptive Preview Chapter Markers - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: adaptive preview chapter marker handling is intentional and proven:
either collected markers are emitted into preview artifacts, or marker
collection is removed because preview chapters are out of scope.

Acceptance signal: the TODO in preview state is resolved, tests prove the
chosen behavior, and preview output truth matches the UI's claims.

## Progress

- [x] 2026-05-26: Audit validated original item `5` as an Audio Engine preview
  behavior workblock.
- [ ] Decide whether preview artifacts should include collected chapter markers.
- [ ] Implement and prove the chosen behavior.

## Surprises & Discoveries

- Observation: preview state stores chapter markers and has a TODO to wire
  marker emission or remove collection.
  Evidence: `src-tauri/src/audio/processor/preview_state.rs`.
- Observation: current engine code appears to log marker counts rather than
  emit preview chapters.
  Evidence: `src-tauri/src/audio/processor/engine.rs`.
- Observation: encoder context intentionally skips chapter passthrough for
  preview mode.
  Evidence: `src-tauri/src/audio/processor/encoder/context.rs` and external FDK
  preview tests.

## Three-Order Trace / Blast Radius

- Order 1, unresolved preview rule:
  preview chapter markers are collected while preview encoder paths skip normal
  chapter passthrough.
- Order 2, immediate blast radius:
  preview state, processor engine logging, encoder context, external FDK
  preview behavior, metadata passthrough policy, and preview artifact tests.
- Order 3, downstream effects:
  ABB can accumulate dead preview state or imply chapter truth that artifacts
  do not contain, making future preview UX and media verification harder to
  trust.

## Decision Log

- Decision: Treat this as preview artifact truth, not monolith cleanup.
  Rationale: resolving the TODO changes or confirms user-visible preview output
  behavior.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Audio Engine preview/processor internals:
    `src-tauri/src/audio/processor/*`.
  - Metadata passthrough policy:
    `src-tauri/src/metadata/*` if chapter metadata is written.
  - Output Artifact Plan / Commit:
    `src-tauri/src/output_artifact/*` if preview artifact commit behavior is
    affected.
- Canon surfaces this spec must not redefine:
  - Processing adapters produce media artifacts; final results report what
    actually happened.
  - Metadata Outcome Plan owns metadata write/passthrough policy.

## Scope And Constraints

In scope:

- Decide whether adaptive previews should include preview chapter markers.
- If yes, emit markers into preview artifacts through the owning metadata/media
  path.
- If no, remove marker collection and update tests/comments accordingly.
- Add tests proving chosen preview marker behavior.

Out of scope:

- External FDK decomposition.
- Terminal outcome classification.
- General preview UX redesign.
- Full chapter authoring feature work.

Constraints:

- Do not imply preview chapters exist in UI/status unless artifacts actually
  contain them.
- Do not bypass Metadata Outcome Plan if writing metadata into preview
  artifacts.
- Keep behavior consistent across native and external FDK preview paths.

## Plan Of Work

- Edits:
  - Trace current preview marker collection through preview state and processor
    engine.
  - Decide and document in code tests whether markers should be emitted or
    removed.
  - Implement marker emission or removal with narrow changes.
  - Update tests for preview artifact metadata expectations.
- Proof steps:
  - Rust processor tests for preview mode marker behavior.
  - Media fixture/probe test if needed to prove chapter metadata exists or is
    intentionally absent.
  - `bun scripts/proof/runner.ts review` for runtime behavior changes.
- Expected repo-visible outcome:
  - No dead marker collection path remains, and preview artifact truth is
    testable.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/audio/processor/preview_state.rs`
  - `src-tauri/src/audio/processor/engine.rs`
  - `src-tauri/src/audio/processor/encoder/context.rs`
  - `src-tauri/src/audio/processor/external_fdk.rs`
- Libraries/external behavior:
  - May need media metadata probing with existing tooling if tests inspect real
    preview artifacts.
- Dependency constraints:
  - Prefer existing media fixtures/tooling before adding dependencies.

## Proof Path and Checks

- Targeted checks:
  - Rust audio processor tests around preview mode.
  - Fixture/probe verification if writing preview chapter markers.
- Full gate:
  - `bun scripts/proof/runner.ts review`.
- Manual or visual evidence:
  - Not required unless UI preview display changes.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill only enduring preview-artifact behavior into canon if future agents
  need a stable rule.
