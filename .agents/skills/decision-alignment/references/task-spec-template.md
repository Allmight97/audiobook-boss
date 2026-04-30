# Task Spec Template

Use this template when `decision-alignment` determines that substantial work
needs one active `docs/specs/<task>.md` implementation spec.

Task specs are temporary working state. Reuse one spec for the same effort,
keep `docs/specs/` flat, and delete the spec after implementation, review,
validation, documentation alignment, and sync are complete.

## Purpose / Big Picture

- What problem is being solved.
- Why this work matters now.
- What good looks like when finished.

## Scope And Constraints

- In-scope outcomes.
- Explicit non-goals.
- Safety, contract, compatibility, or policy boundaries.

## Solution Posture

- Chosen posture: local patch, subsystem refactor, or redesign.
- Why that posture fits this effort.
- Any narrower option rejected because it preserves a bad seam, malformed
  contract, or local minimum.
- What would justify broadening or narrowing scope later.

## Context And Orientation

- Owning files, commands, tests, and docs.
- Current implementation shape relevant to the task.
- Boundary ownership and invariants protected.

## Plan Of Work

- Work phases or milestones.
- Concrete task breakdown.
- Sub-agent lanes when useful.

## Progress

- Timestamped status notes for meaningful completed work.
- Enough detail to resume without rereading chat.

## Surprises And Discoveries

- Facts learned during execution that changed the plan.
- Failed avenues or constraints discovered from repo/tooling evidence.

## Accepted Decisions

- Decisions made and why they were chosen.
- Reversals when the earlier plan proved wrong.
- Alternatives only when they prevent likely re-litigation.

## Validation And Acceptance

- Exact commands and artifacts needed to claim done.
- Review-agent requirements when relevant.
- Documentation-alignment requirements.

## Interfaces And Dependencies

- User-visible behavior changes.
- Contract, schema, command, doc, or tooling surfaces that must stay aligned.

## Idempotence And Recovery

- Safe restart points.
- Cleanup or retry guidance if interrupted.
- What can be rerun without damage.

## Completion And Cleanup

- What must be true before deleting the spec.
- Note that the spec is deleted after completion, not archived.
