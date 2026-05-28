# Processing Terminal Truth Split - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: processing terminal outcome logic remains behaviorally identical but
is easier to audit, extend, and prove. Classification, skip paths, and batch
aggregation become named private modules or helpers behind the existing
processing public strip.

Acceptance signal: every processing job still has exactly one terminal outcome
(`success`, `skipped`, `cancelled`, or `failed`), Status Panel truth is still
backend-derived, and focused tests cover merge/batch/cancel/skip edge cases.

## Progress

- [x] 2026-05-26: Audit validated original item `3` as a processing
  architecture/refactor workblock.
- [ ] Split internal terminal-outcome responsibilities without changing public
  event contracts.
- [ ] Add or preserve integration tests for edge cases.

## Surprises & Discoveries

- Observation: `terminal_outcomes.rs` is large, but much of the line count is
  inline tests.
  Evidence: `src-tauri/src/processing/terminal_outcomes.rs`.
- Observation: production responsibility is still concentrated: terminal class
  selection, all-skipped/no-write paths, failure/cancel helpers, and batch
  aggregation live together.
  Evidence: `src-tauri/src/processing/terminal_outcomes.rs`.
- Observation: `run.rs` also concentrates preflight, queue/finalize, batch skip
  scheduling, per-job terminal mapping, and inline tests.
  Evidence: `src-tauri/src/processing/run.rs`.

## Three-Order Trace / Blast Radius

- Order 1, concentrated terminal rules:
  terminal classification, skip/no-write result construction, cancel/failure
  helpers, and batch aggregation live in oversized processing modules.
- Order 2, immediate blast radius:
  `run.rs` orchestration, Job Registry lifecycle, progress event emission,
  batch skip short-circuiting, and Status Panel terminal result consumption.
- Order 3, downstream effects:
  a classification regression can produce false success, false failure,
  missing cancellation truth, or inconsistent batch summaries that the UI will
  faithfully render as backend truth.

## Decision Log

- Decision: Treat this as Processing/Backend Lifecycle truth, not an Audio
  Engine or Status Panel refactor.
  Rationale: Status Panel must consume backend terminal truth, and Audio Engine
  should not own final UI outcome classification.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Processing Plan and Backend Lifecycle:
    `src-tauri/src/processing/*`.
  - Job Registry:
    `src-tauri/src/processing/job_registry/*`.
  - Status Panel consumer:
    `src/ui/statusPanel/*`.
- Canon surfaces this spec must not redefine:
  - Exactly one terminal outcome per processing job.
  - UI renders backend terminal truth.
  - Event stage shape is generated from Rust.

## Scope And Constraints

In scope:

- Internal split of terminal outcome classification.
- Internal split of skip/no-write result construction.
- Internal split of batch aggregation and merge behavior.
- Moving inline tests into clearer sibling test modules if that improves scan
  quality.
- Edge-case tests for cancellation, failure, all-skipped, partial batch, and
  merge behavior.

Out of scope:

- Encoder settings validation/capability changes.
- External FDK process lifecycle refactor.
- Status Panel redesign.
- New terminal outcome categories.

Constraints:

- Do not change generated event vocabulary unless explicitly required and
  proven through binding regeneration.
- Do not introduce a second lifecycle tracker outside Job Registry.
- Long-running job cancellation semantics must remain truthful.

## Plan Of Work

- Edits:
  - Identify the smallest internal module split that gives named ownership:
    classifier, skip paths, batch aggregation, and result helpers.
  - Keep existing public functions or add a narrow public strip inside
    `processing` if needed.
  - Move tests near their owned behavior without losing coverage.
  - Add integration-level tests for merge/batch/cancel edge cases before or
    during the split.
- Proof steps:
  - Focused Rust processing tests.
  - Binding check if event types are touched.
  - `mise run proof`.
- Expected repo-visible outcome:
  - Terminal outcome behavior is easier to review without weakening backend
    truth guarantees.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/processing/terminal_outcomes.rs`
  - `src-tauri/src/processing/run.rs`
  - `src-tauri/src/processing/progress/*`
  - `src-tauri/src/processing/job_registry/*`
- Libraries/external behavior:
  - None.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust tests covering terminal outcome classification.
  - Rust tests covering batch aggregation and skip/cancel/failure behavior.
  - Generated binding drift check if event shapes change.
- Full gate:
  - `mise run proof`.
- Manual or visual evidence:
  - Not required unless Status Panel presentation changes.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill enduring terminal-outcome ownership language only if the public strip
  or nested `AGENTS.md` needs clarification.
