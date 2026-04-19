# Planning Contract

Use this file for the planning protocol only. It is durable because it defines how
long-horizon work is planned in this repo; it is not a task log and it should not
change when a specific effort progresses.

The goal is simple: if a fresh agent opens this repo and is asked to create or
continue a substantial plan, it should be able to follow this contract and produce
or resume a task spec that is better than generic default planning.

## Stable vs Ephemeral

- `PLANS.md` is stable.
  It changes only when this planning protocol changes.
- `docs/specs/<task>.md` is ephemeral.
  It is one living implementation spec for one substantial effort.
- A task spec is reused across sessions for the same effort.
- A task spec is never created per session.
- Completed task specs are deleted. Do not archive them in-repo.

## When A Task Spec Is Required

Create `docs/specs/<task>.md` when work is substantial enough that chat alone is
not a trustworthy source of implementation state. Typical triggers:

- multi-step work that spans multiple sessions
- work split across sub-agents or parallel lanes
- changes with meaningful verification or review gates
- work where “claimed done” vs actual done can drift
- work that touches multiple subsystems or several canon surfaces

Do not create a task spec for small single-pass fixes, trivial docs edits, or work
that can be completed safely inside one short session without state drift.

## Fresh-Session Routing

When a fresh agent is asked to plan or continue substantial work:

1. Read `AGENTS.md`.
2. Read this file before drafting or revising a substantial plan.
3. Reuse an existing `docs/specs/<task>.md` for the same effort if one exists.
4. Only create a new task spec when no matching active spec exists.

## Required Standards For docs/specs/<task>.md

Each active task spec must be self-contained enough that another capable engineer
or agent could continue the work with only the working tree and the spec file.

Write the spec in prose-first form. Do not turn it into terse TODO fragments that
lose context. Another agent should not need this chat thread to understand the
intent, constraints, current status, and validation path.

Every active task spec must contain these sections:

### 1. Purpose / Big Picture

- what problem is being solved
- why this work matters now
- what “good” looks like when finished

### 2. Scope And Constraints

- in-scope outcomes
- explicit non-goals
- hard constraints, compatibility requirements, or policy boundaries

### 3. Solution Posture

- chosen posture: local patch, subsystem refactor, or redesign
- why that posture is preferred for this effort
- whether a narrower option was rejected because it would preserve a bad seam, malformed contract, or other local minimum
- what would justify broadening or narrowing scope later

### 4. Context And Orientation

- exact files, commands, tests, and docs that matter
- current implementation shape relevant to the task
- boundary ownership to prevent wandering discovery

### 5. Plan Of Work

- grouped work phases or milestones
- concrete task breakdown for execution
- sub-agent lane breakdown when applicable

### 6. Progress

- timestamped checklist or status notes for meaningful completed work
- enough detail to resume without rereading chat transcripts

### 7. Surprises And Discoveries

- facts learned during execution that materially changed the plan
- failed avenues or constraints discovered from the repo/tooling

### 8. Decision Log

- decisions made, with the reason they were chosen
- include reversals when the earlier plan proved wrong

### 9. Validation And Acceptance

- exact commands and artifacts needed to claim the work is done
- include review-agent requirements when the task calls for one
- include documentation-alignment requirements

### 10. Interfaces And Dependencies

- user-visible behavior changes
- contract or schema changes
- commands, docs, or tooling surfaces that must stay aligned

### 11. Idempotence And Recovery

- cleanup or retry guidance if the work is interrupted
- safe restart points and what can be rerun without damage

### 12. Completion And Cleanup

- what must be true before the task spec can be deleted
- note that the file is deleted after full completion, not archived

## ABB-Specific Expectations

Task specs for Audiobook Boss should explicitly call out the repo-specific proof
surfaces instead of leaving them implicit:

- docs-only changes use `bash scripts/check-context-surface.sh`
- non-doc code/config changes use `scripts/checks.sh standard`
- UI-affecting changes should say whether `bun run harness:verify --changed` is required
- contract work should say whether bindings, boundary adapters, and related tests must move together
- fallback work should say whether `docs/fallbacks.md` and `scripts/check-fallback-policy.sh` are part of acceptance
- planning should name any canonical docs that must be updated on completion, such as `AGENTS.md`, `README.md`, `docs/api-map.md`, or `docs/fallbacks.md`
- planning should explicitly note when malformed seams, contract drift, or bad solution shape were discovered and whether the task intentionally fixes them now or defers them

## Working Rules

- The orchestrating/main agent owns updates to the task spec.
- Sub-agents use the task spec as read-only context and report results back for
  consolidation.
- Update the existing task spec in place as decisions settle or tasks complete.
- If the implementation meaningfully diverges from the plan, fix the spec first or
  explicitly record the divergence before claiming completion.
- Keep prose dense and useful, but not historical for its own sake.

## Completion And Cleanup

A task spec remains active until the full flow is done:

1. idea -> discuss/spec/plan
2. implement, including sub-agent work where useful
3. dedicated review-agent pass against the spec/tasks when appropriate
4. validation and test gates pass
5. project documentation is aligned to the landed behavior
6. commit/push/PR-ready state is reached
7. delete `docs/specs/<task>.md`

Do not “promote” plan text into canon as part of cleanup. If implementation needs
durable docs changes, update the normal canonical docs directly.
