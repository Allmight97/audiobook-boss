# Processing Terminal Truth Split - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose

Outcome: processing terminal outcome logic remains behaviorally identical but
is easier to audit, extend, and prove. Classification, skip/no-write paths, and
batch aggregation become named private modules or helpers behind the existing
processing public strip.

Acceptance signal: every processing job still has exactly one terminal outcome
(`success`, `skipped`, `cancelled`, or `failed`), Status Panel truth is still
backend-derived, and focused tests cover merge, batch, cancel, skip, and missing
terminal-result edge cases.

## Current Evidence

- `src-tauri/src/processing/terminal_outcomes.rs` owns terminal class
  selection, skip/no-write result construction, cancellation-vs-failure
  normalization, missing-result repair, batch aggregation, failure event
  collection, and a large inline test block.
- `src-tauri/src/processing/run.rs` owns preflight, queue dispatch,
  all-skipped short-circuiting, per-job result mapping, registry
  complete/fail transitions, terminal event emission, and inline tests.
- `src-tauri/src/processing/job_registry/AGENTS.md` requires every queued item
  to reach terminal truth so UI state never hangs on missing indices.
- `docs/system-map.md` defines Backend Lifecycle as a processing sub-owner, not
  a separate Grey-Box Public API.

## Decision Log

- Decision: keep this as a Processing/Backend Lifecycle workblock.
  Rationale: Status Panel should consume backend terminal truth, and Audio
  Engine should not own final UI outcome classification.
  Date: 2026-05-28.
- Decision: do not merge this with the Audio Engine private-cluster workblock.
  Rationale: Audio execution reports media work; processing owns queue,
  cancellation, skip, failure, and batch terminal normalization.
  Date: 2026-05-28.

## Scope

In scope:

- Split terminal outcome classification into a clearer private owner.
- Split skip/no-write result construction into a clearer private owner.
- Split batch aggregation, missing-result repair, and failure-event collection
  into a clearer private owner.
- Move bulky inline tests into sibling test modules if that improves scan
  quality without widening visibility.
- Add or preserve tests for cancellation, failure, all-skipped, partial batch,
  missing-result repair, and merge behavior.

Out of scope:

- Encoder settings validation/capability changes.
- External FDK process lifecycle refactor.
- Status Panel redesign.
- New terminal outcome categories.
- New lifecycle tracker outside Job Registry.

Constraints:

- Do not change generated event vocabulary unless explicitly required and
  proven through binding regeneration.
- Do not introduce a second lifecycle tracker outside Job Registry.
- Long-running job cancellation semantics must remain truthful.
- UI remains a terminal-truth consumer, not a terminal-truth author.

## Plan Of Work

- Identify the smallest internal module split that gives named ownership:
  classifier, skip/no-write paths, batch aggregation/repair, and event helpers.
- Keep existing public functions stable or add a narrow internal public strip
  inside `processing` only when it reduces reach-through.
- Move tests near their owned behavior without losing coverage.
- Add focused tests for merge/batch/cancel/skip/missing-result cases before or
  during the split.
- Keep behavior changes out unless a test exposes a real bug; if behavior must
  change, record the terminal truth mutation in this spec before implementation
  proceeds.

## Proof Path

- Focused Rust processing tests for terminal classification.
- Focused Rust tests for batch aggregation and skip/cancel/failure behavior.
- Generated binding drift check if event shapes change.
- `scripts/check-public-api-strips.sh` if visibility changes.
- `bun scripts/proof/runner.ts review` before handoff.

## Cleanup Trigger

When implemented, reviewed, validated, docs-aligned, and synced:

- Delete this spec.
- Distill enduring terminal-outcome ownership language only if the processing
  public strip or nested `AGENTS.md` needs clarification.
