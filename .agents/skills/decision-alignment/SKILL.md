---
name: decision-alignment
description: ABB-specific repo-grounded alignment for architecture, product, fallback/compat, domain-language, spec, issue, doc, and proof-path decisions. Use when an Audiobook Boss discussion depends on current repo truth or may need a spec, issue, doc update, decision note, or validation route. Inspect repo artifacts first, ask no more than two action-changing questions, and route accepted decisions to the smallest ABB surface.
---

# Decision Alignment

## Overview

Use this skill to turn an ABB design, architecture, or product conversation into
an aligned decision, bounded non-decision, active spec, issue, doc route, or
proof step.

Default to a proposed decision, patch shape, or proof route. Do not edit durable
repo docs unless the user asks for the change.

## Boundary

Use this for ABB-specific decisions that depend on current repo truth, domain
language, fallback policy, specs, issues, docs, code boundaries, or durable
capture.

Do not use this for generic pressure-testing, personal confidence calibration,
non-ABB decisions, or strategy work whose primary need is a roadmap artifact.

## Core Loop

1. Name the decision in one sentence.
2. Locate the affected ABB layer using `docs/system-map.md`: Product intent, UI
   state, IPC contract, Backend lifecycle, or Artifact truth.
3. Inspect repo artifacts before asking for facts the repo can answer: start
   with `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`,
   `docs/fallbacks.md`, and owning code/docs for the touched boundary.
4. Ask one or two action-changing questions at a time. Each question must move
   toward coherence, alignment, a locked decision, a proof path, or a concrete
   next action. Include recommended answers.
   Treat two questions as a hard cap: if more facts would help, combine them or
   choose the two that change routing or implementation readiness.
   A list of requested facts counts as questions; do not use it to bypass the
   cap.
5. Keep going until the accepted decision, non-decision, proof path, spec/issue
   route, or implementation-ready shape is clear.

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
| Active substantial implementation state | `docs/specs/<task>.md` via Spec Handoff |
| Fallback, shim, or compatibility behavior | `docs/fallbacks.md` plus source marker and `scripts/check-fallback-policy.sh` |
| Stable product/system ownership shape | `docs/system-map.md` |
| Canonical term or overloaded language fix | `docs/ubiquitous-language.md` |
| Deferred work or revisit-later concept | GitHub issue |
| Accepted durable rationale with no better home and value after active spec cleanup | `docs/decisions.md` escape hatch |
| Personal learning | outside repo unless explicitly requested |
| Discarded ideas or transient reasoning | keep out of repo docs |

Keep `docs/fallbacks.md` separate from general decisions. Fallbacks need active
enforcement, source markers, sunset dates, and renewal rules.

## Spec Handoff

Create or update one active `docs/specs/<task>.md` when work is substantial,
multi-session, multi-agent, cross-subsystem, has meaningful verification gates,
or could drift between claimed done and actually done.

Do not create a spec for small single-pass fixes, trivial docs edits, or work
that can finish safely in one short session.

A spec is working state, not canon history. Reuse the existing spec for the same
effort, keep `docs/specs/` flat, and delete the spec once implementation,
review, validation, doc alignment, and sync are complete.

When creating or revising a spec, use
`references/task-spec-template.md`. Keep the spec decision-complete rather than
historical; another agent should be able to resume from repo plus spec without
rereading chat.

## Decision Note Escape Hatch

Use `docs/decisions.md` only for accepted ABB rationale that should survive
active spec cleanup and has no better home. It is not a normal docs surface, ADR
tree, changelog, issue tracker, PR recap, historical archive, or learning log.

Prefer existing homes first: active spec, `docs/fallbacks.md`,
`docs/system-map.md`, `docs/ubiquitous-language.md`, GitHub issue, release notes,
or changelog.

Before proposing a decision note, answer in chat:

- Which accepted decision would be expensive or confusing to rediscover later?
- Why is the rationale not better housed in an existing ABB surface?
- What future agent or human behavior will the entry change?

If `docs/decisions.md` does not exist, propose the first entry in chat and
create it only when the user asks. If it exists, append one compact accepted
decision entry, newest first.

Use this shape:

```md
## DEC-YYYY-MM-DD-short-title

Date: YYYY-MM-DD
Scope: Product intent | UI state | IPC contract | Backend lifecycle | Artifact truth

Decision:
One or two sentences stating the accepted choice.

Why:
The concise rationale future agents need.

Alternatives:
- Include only alternatives that explain a real trade-off or tempting wrong turn.

Consequences:
What this makes easier, harder, or intentionally out of scope.

Evidence:
- PR, issue, code path, test, release, or canon doc proving the decision is real.

Revisit Trigger:
The concrete condition that should make ABB reconsider the decision.
```

If creating or updating `docs/decisions.md`, keep the file short and run
`bash scripts/check-context-surface.sh`.

## Output Shape

For short alignment loops, answer in prose with the next question.

For substantial decisions, end with:

- **Aligned Decision**: accepted choice or current best recommendation.
- **Why It Holds**: product, architecture, or domain reason.
- **Boundary / Layer**: ABB layer and owning files/docs.
- **Proof Path**: checks, tests, review evidence, or artifacts needed.
- **Routing**: no doc change, proposed issue/spec, or proposed doc patch.
- **Next Question**: one remaining high-leverage question, if any.

If the conversation exposes a surprising architecture trap, apply the Canary rule
from `AGENTS.md`: name the trap, affected boundary, assumption used to continue,
and smallest doc change that would prevent recurrence.
