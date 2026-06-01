---
name: decision-alignment
description: ABB-specific repo-grounded alignment for architecture, product, fallback/compat, domain-language, active spec, issue, doc, and verification-path decisions. Use when an Audiobook Boss discussion depends on current repo truth or may need an active spec, issue, doc update, decision note, or validation route. Align decisions to ABB's clean, agent-first repo north star, inspect repo artifacts first, ask no more than two action-changing questions, and route accepted decisions to the smallest ABB surface.
---

# Decision Alignment

## Overview

Use this skill to turn an ABB design, architecture, or product conversation into
an aligned decision, bounded non-decision, active spec, issue, doc route, or
verification step.

Default to a proposed decision, patch shape, or verification route. Do not edit durable
repo docs unless the user asks for the change.

## North Star: Clean, agent-first codebase and repo
- Verification infrastructure and docs that reflect current repo truth.
- Collaborative human feedback as scope control.
- Temporary planning state purged or distilled after use.
- Smallest coherent home future agents must consult.
- Clear feedback loops for Agentic Experience.

## Boundary

Use this for ABB-specific decisions that depend on current repo truth, domain
language, fallback policy, active specs, issues, docs, code boundaries, or
durable capture.

Do not use this for generic pressure-testing, personal confidence calibration,
non-ABB decisions, or strategy work whose primary need is a roadmap artifact.

## Core Loop

1. Reword the decision as a "north star" - the goal or objective that the decision is trying to achieve.
2. Verify the affected ABB layer starting with `docs/system-map.md`: Product intent, UI
   state, IPC contract, Backend lifecycle, or Artifact truth.
3. Inspect repo elements before asking for facts the repo can answer: start
   with `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`,
   `docs/fallbacks.md`, and owning code/docs for the touched boundary.
4. Ask no more than two sharp questions aiming toward coherence, alignment, a locked decision, a verification path, or a concrete
   next action. Include recommended answers.
   Treat two questions as a hard cap: if more facts would help, combine them or
   choose the two that change routing or implementation readiness.
   A list of requested facts counts as questions; do not use it to bypass the
   cap.
5. Keep going until the alignment is defined enough for a roadmap, and/or
   active-spec/issue route, or implementation-ready shape is clear enough to hand off.

## Challenge Rules

- **Challenge fuzzy language**: propose ABB terms when wording is vague,
  overloaded, or conflicts with `docs/ubiquitous-language.md`.
- **Probe concrete scenarios**: name the actor, input, boundary crossed,
  expected terminal outcome, and falsifying evidence.
- **Cross-check claims**: inspect owning code, tests, or canon docs before
  treating claims about ABB behavior as true.
- **Surface contradictions**: when code, docs, tests, or conversation disagree,
  name the contradiction and ask which source should change.
- **Separate personal learning from repo truth**: keep learning notes outside
  repo docs unless they clarify ABB product or system ownership.

## Decision Capture

Recommend durable capture only for accepted decisions that pass at least one
test:

- **Hard to reverse**: changing course later has meaningful cost.
- **Surprising without context**: a future maintainer would wonder why ABB chose
  this shape.
- **Real trade-off**: ABB chose between plausible alternatives for a specific
  reason.

Do not record discarded ideas for their own sake. Mention alternatives only when
they prevent likely re-litigation, explain a real trade-off, or clarify why a
tempting path is wrong for ABB. Put deferred ideas or revisit-later concepts in
GitHub issues, not durable decision notes.

## Routing

Route the aligned outcome to the smallest existing home future work must
consult:

| Outcome | Preferred home |
| --- | --- |
| Temporary multi-session planning, roadmap, architecture, or implementation state | `docs/specs/<task>.md` via Active Spec Handoff |
| Fallback, shim, or compatibility behavior | `docs/fallbacks.md` plus source marker and `scripts/check-fallback-policy.sh` |
| Stable product/system ownership shape | `docs/system-map.md` |
| Canonical term or overloaded language fix | `docs/ubiquitous-language.md` |
| Deferred work or revisit-later concept | GitHub issue |
| Accepted durable rationale with no better home and value after active-spec cleanup | `docs/DECISIONS.md` escape hatch |
| Personal learning | outside repo unless explicitly requested |
| Discarded ideas or transient reasoning | keep out of repo docs |

Keep `docs/fallbacks.md` separate from general decisions. Fallbacks need active
enforcement, source markers, sunset dates, and renewal rules.

## Tool Routing

Use other skills as tools only when they change the alignment outcome:

- Use global `improve-codebase-architecture` / Architecture Scout when ABB
  alignment needs bounded repo-grounded architecture or refactor candidates
  before deciding direction.
- Use global `implementation-handoff` when the decision, roadmap, or design is
  aligned enough to hand to a tactical implementation agent.
- Use `abb-library-research` when the decision depends on current external
  library/API behavior or vendored reference source.
- Use focused ABB execution skills when the aligned decision touches their
  invariant: metadata, IPC contracts, path safety, job lifecycle, dependencies,
  release, or resource lifetime.

Stay in this skill when the live question is whether ABB should do the work,
what outcome should hold, or where accepted context belongs.

## Active Spec Handoff

Create or update `docs/specs/<task>.md` when planning, roadmap, architecture, or
implementation work is substantial, multi-session, multi-agent,
cross-subsystem, has meaningful verification gates, or could drift between
claimed done and actually done.

Do not create an active spec for small single-pass fixes, trivial docs edits, or
work that can finish safely in one short session.

Active spec rules:

- Temporary work state, not feature canon, transcript history, or a session
  ledger.
- Reuse the existing spec for the same effort.
- Keep `docs/specs/` flat.
- Delete the spec or distill enduring truths into canon once implementation,
  review, validation, docs alignment, and sync are complete.

When creating or revising an active spec, use
`references/active-spec-template.md`.

Required active-spec qualities:

- self-contained enough to resume without prior chat context
- current progress, discoveries, decisions, verification status, and remaining work
- observable outcomes
- concrete repo paths and commands
- explicit cleanup trigger: delete, or distill concise enduring rules into canon

## Decision Note Escape Hatch

Use `docs/DECISIONS.md` only for accepted ABB rationale that should survive
active-spec cleanup and has no better home. It is not a normal docs surface,
ADR tree, changelog, issue tracker, PR recap, historical archive, or learning
log.

Prefer existing homes first: active spec, `docs/fallbacks.md`,
`docs/system-map.md`, `docs/ubiquitous-language.md`, GitHub issue, release notes,
or changelog.

Before proposing a decision note, answer in chat:

- Which accepted decision would be expensive or confusing to rediscover later?
- Why is the rationale not better housed in an existing ABB surface?
- What future agent or human behavior will the entry change?

If `docs/DECISIONS.md` does not exist, propose the first entry in chat and
create it only when the user asks. If it exists, append one compact accepted
decision entry, newest first.

When creating or appending an entry, load
`references/decision-note-template.md`.

If creating or updating `docs/DECISIONS.md`, keep the file short and verify the
references you add against current repo state.

## Output Shape

For short alignment loops, answer in prose with the next question.

For substantial decisions, end with:

- **Aligned Decision**: accepted choice or current best recommendation.
- **Why It Holds**: product, architecture, or domain reason.
- **Boundary / Layer**: ABB layer and owning files/docs.
- **Verification Path**: checks, tests, review evidence, or artifacts needed.
- **Routing**: no doc change, proposed issue/active spec, or proposed doc patch.
- **Next Question**: one remaining high-leverage question, if any.

If the conversation exposes a surprising architecture trap, name the trap,
affected boundary, assumption used to continue, and smallest doc change that
would prevent recurrence.
