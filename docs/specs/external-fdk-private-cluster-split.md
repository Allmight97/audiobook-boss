# External FDK Private Cluster Split - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: external FDK encoding remains behaviorally stable while its highest
risk private mechanisms are split into reviewable modules: argument building,
spawn/monitor/kill, progress parsing, staging/finalization coordination, and
fixtures/tests.

Acceptance signal: the public entrypoint stays stable, cancellation and cleanup
semantics remain true, tests cover args/progress/cancel/error paths, and future
encoder/toolchain changes no longer require reviewing one oversized file.

## Progress

- [x] 2026-05-26: Audit validated original item `4` as an Audio Engine private
  cluster workblock.
- [ ] Split internal responsibilities behind the existing processor surface.
- [ ] Prove cancellation, cleanup, args, and progress behavior after the split.

## Surprises & Discoveries

- Observation: `external_fdk.rs` is the largest first-party file, though much
  of the line count is inline tests.
  Evidence: `src-tauri/src/audio/processor/external_fdk.rs`.
- Observation: production logic mixes temp staging, cleanup ownership,
  spawn/monitor/kill, progress parsing, cancellation, metadata passthrough,
  argument building, and finalization orchestration.
  Evidence: `src-tauri/src/audio/processor/external_fdk.rs`.
- Observation: cancellation and cleanup patterns already exist; the issue is
  change risk and reviewability, not a confirmed active leak.
  Evidence: `CleanupGuard` and termination helpers in
  `src-tauri/src/audio/processor/external_fdk.rs`.

## Three-Order Trace / Blast Radius

- Order 1, concentrated execution mechanisms:
  temp staging, cleanup guard ownership, argument building, child process
  spawn/monitor/kill, progress parsing, cancellation, passthrough metadata, and
  inline fixtures/tests live in one module.
- Order 2, immediate blast radius:
  external FFmpeg command correctness, cancellation cleanup, progress events,
  output artifact finalization, metadata passthrough, and processor error
  mapping.
- Order 3, downstream effects:
  future encoder/toolchain changes can accidentally leave temp residue, orphan
  processes, misleading progress, corrupt output artifacts, or hard-to-review
  cancellation behavior.

## Decision Log

- Decision: Treat this as Audio Engine private cluster decomposition, not a
  public API redesign.
  Rationale: callers should still depend on a small processor surface while
  private execution mechanisms become easier to test.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Audio Engine Deep Module:
    `src-tauri/src/audio/processor/*`.
  - Output Artifact Plan / Commit:
    `src-tauri/src/output_artifact/*`.
  - Processing lifecycle consumer:
    `src-tauri/src/processing/*`.
- Canon surfaces this spec must not redefine:
  - Processor code delegates final artifact commit policy to
    `output_artifact`.
  - Cancellation and cleanup paths must emit truthful terminal state.
  - No path validation or artifact commit policy is recreated inside
    processor-private modules.

## Scope And Constraints

In scope:

- Extract external FDK argument building.
- Extract spawn/monitor/kill/wait logic.
- Extract progress parsing.
- Extract test fixtures/helpers from inline tests where useful.
- Preserve staging, cleanup, cancellation, and metadata passthrough behavior.

Out of scope:

- Terminal outcome classification split.
- New encoder feature work.
- Preview chapter marker behavior changes.
- Output artifact commit policy changes.

Constraints:

- Keep existing public processor entrypoint stable unless a narrow internal
  public strip improvement is required.
- Preserve cleanup ownership transitions; remove cleanup ownership only after
  durable artifact commit.
- Kill and wait external processes deterministically on cancellation/error.
- Do not bypass `output_artifact` finalization ownership.

## Plan Of Work

- Edits:
  - Create sibling private modules under `src-tauri/src/audio/processor/` for
    external FDK args, process monitoring, progress parsing, and test fixtures
    as needed.
  - Move code mechanically first, then make only small local readability
    improvements.
  - Add focused tests around arg construction, progress parsing, child process
    termination classification, and cleanup ownership.
  - Keep behavior changes out unless a test exposes a real bug.
- Proof steps:
  - Focused Rust audio processor tests.
  - Cancellation/cleanup tests where deterministic without real media.
  - `bun scripts/proof/runner.ts review`.
- Expected repo-visible outcome:
  - External FDK work is reviewable by mechanism without changing user-visible
    encoding behavior.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/audio/processor/external_fdk.rs`
  - Potential new private siblings under
    `src-tauri/src/audio/processor/external_fdk/` or equivalent.
  - `src-tauri/src/output_artifact/*`
  - `src-tauri/src/processing/progress/*`
- Libraries/external behavior:
  - External FFmpeg/FDK process behavior must be abstracted or fixture-tested
    where possible.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust tests for external FDK argument construction.
  - Rust tests for progress parsing.
  - Rust tests for cancellation/error cleanup semantics where deterministic.
- Full gate:
  - `bun scripts/proof/runner.ts review`.
- Manual or visual evidence:
  - Not required unless implementation changes actual encoder behavior.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill any enduring resource-lifetime rule into the nearest
  `AGENTS.md` only if future processor work must obey a new explicit pattern.
