---
name: decision-alignment
description: ABB-specific doc-aware decision alignment and planning pressure-test. Use when an Audiobook Boss discussion depends on current repo truth, domain language, fallback/compat policy, issue/spec routing, or durable ABB capture. Inspect repo artifacts before asking for known facts, ask no more than two action-changing questions at a time, and route only accepted decisions into the smallest existing ABB surface; use the `docs/decisions.md` escape hatch only for rare accepted rationale that should survive active spec cleanup and has no better home.
---

# Decision Alignment

## Overview

Use this skill to turn an ABB design conversation into an aligned decision,
bounded non-decision, or concrete next proof step. It is a repo-grounded
conversation mode, not a generic plan generator and not an ADR system.

Default to a proposed patch or decision summary. Do not edit durable repo docs
unless the user explicitly asks to write the change.

This repo-local skill is intentionally ABB-specific. The reusable pattern is
decision pressure-test plus project-truth inspection plus durable routing. If a
future repo needs the same pattern, extract or create a global/base
`decision-alignment` skill and keep repo-specific routing in that repo; do not
weaken this ABB skill by making ABB artifact requirements generic.

## Relation To Global `grill-me`

Keep global `grill-me` as the project-agnostic pressure-test skill for personal
plans, general design reviews, and non-ABB decisions. Do not replace it with a
global `grill-me-with-docs` clone.

This skill is the ABB-specific doc-aware specialization: it uses the same sharp,
one-or-two-question-at-a-time posture, but it must inspect ABB code/docs before
asking for known facts and must route accepted decisions into ABB's existing
surfaces.

Use `grill-me` when no ABB-specific code, docs, issue/spec routing, fallback
policy, or domain language is involved. Use `decision-alignment` when the answer
depends on ABB's current repo truth or may need durable ABB capture.
If the prompt is ABB-related but does not depend on current repo truth or durable
ABB capture, use `grill-me` instead.
Do not trigger this skill for every ABB-flavored conversation. If the user only
wants generic pressure-testing, learning, or personal confidence calibration and
no repo-truth lookup or durable ABB routing is needed, use global `grill-me`.

## Core Loop

1. Name the decision under discussion in one sentence.
2. Locate the affected ABB layer and design bias using `docs/system-map.md`:
   Product intent, UI state, IPC contract, Backend lifecycle, or Artifact truth.
3. Inspect existing repo artifacts before asking the user to restate facts:
   start with `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`,
   `docs/fallbacks.md`, and owning code/docs for the touched boundary.
4. Ask one or two high-leverage questions at a time. The questions do not need
   to be inseparable, but each must be action-changing, accretive, and move ABB
   toward coherence, alignment, a locked decision, a proof path, or a concrete
   next action.
5. Include the recommended answer with each question so the user can agree,
   reject, or refine the decision instead of starting from a blank page.
6. Keep going until one is true:
   - the accepted decision and proof path are clear,
   - the work is ready for implementation,
   - the remaining uncertainty is explicitly bounded,
   - the right next action is a GitHub issue or active task spec,
   - or the user stops the alignment loop.

## Codebase Challenge Rules

Apply these checks during the alignment loop:

- **Challenge fuzzy language**: when a term is vague, overloaded, or conflicts
  with `docs/ubiquitous-language.md`, call it out and propose a sharper ABB term.
- **Probe with concrete scenarios**: when a relationship, workflow, or boundary
  is unclear, invent one specific scenario that forces the edge case into view.
- **Cross-check code and docs**: when the user or agent claims how ABB works,
  inspect the owning code, tests, or canon docs before treating the claim as
  true.
- **Surface contradictions**: if code, docs, tests, or conversation disagree,
  name the contradiction and ask which source should change.
- **Route through ABB surfaces**: use the Routing table before proposing any new
  doc type. Propose a new durable surface only when an accepted decision has no
  adequate existing home.
- **Separate personal learning from repo truth**: route JStar's learning notes
  outside the repo unless they directly clarify ABB product/system ownership.

## Doc-Aware Grilling Protocol

Do not ask the user to restate facts that the repo can answer. Before each
question, decide whether a quick repo read, issue lookup, or source check would
answer it more reliably than conversation.

Use concrete scenarios to force hidden branches into view. A good scenario names
the actor, input, boundary crossed, expected terminal outcome, and proof that
would make the answer falsifiable.

When language is fuzzy, propose the ABB term and explain the boundary it protects.
Recommend `docs/ubiquitous-language.md` updates only when the term is likely to
change future agent behavior.

Prefer questions that decide scope, owner, layer, proof, reversibility, durable
home, user impact, or whether the current uncertainty should become an explicit
non-decision.

When the conversation reveals a durable decision, capture the decision shape in
chat first. Write or update repo artifacts only after the user accepts the shape
or explicitly asks for implementation.

## Decision Capture Test

Recommend durable capture only for accepted decisions that pass at least one
test:

- **Hard to reverse**: changing course later has meaningful cost.
- **Surprising without context**: a future agent or maintainer would wonder why
  ABB chose this shape.
- **Real trade-off**: there were plausible alternatives and ABB chose one for a
  specific reason.

Do not record discarded ideas for their own sake. Mention alternatives only when
they prevent likely re-litigation, explain a real trade-off, or clarify why a
tempting path is wrong for ABB. Put deferred ideas or revisit-later concepts in
GitHub issues, not durable decision notes.

## Routing

Route the aligned outcome to the smallest existing home that future work must
consult:

| Outcome | Preferred home |
| --- | --- |
| Active substantial implementation state | `docs/specs/<task>.md` via Spec Handoff |
| Fallback, shim, or compatibility behavior | `docs/fallbacks.md` plus source marker and `scripts/check-fallback-policy.sh` |
| Stable product/system ownership shape | `docs/system-map.md` |
| Canonical term or overloaded language fix | `docs/ubiquitous-language.md` |
| Deferred work or revisit-later concept | GitHub issue |
| Accepted durable rationale with no better home and value after active spec cleanup | `docs/decisions.md` escape hatch, only after the Decision Note Escape Hatch below |
| JStar (repo owner) personal Learnings | upon request, write directly to `/Users/jstar/Library/Mobile Documents/iCloud~md~obsidian/Documents/Main Vault/Projects/Project Learnings/ABB` |
| Discarded ideas or transient reasoning | keep out of repo docs |

Keep `docs/fallbacks.md` separate from general decisions. Fallbacks are
operational decisions with active enforcement, source markers, sunset dates, and
renewal rules.

## Spec Handoff

When alignment becomes substantial implementation work, create or update one
active `docs/specs/<task>.md` if chat alone is not a trustworthy state holder.

Use a spec when work is multi-step, multi-session, multi-agent, touches multiple
subsystems, has meaningful verification gates, or could drift between claimed
done and actually done.

Do not create a spec for small single-pass fixes, trivial docs edits, or work
that can finish safely in one short session.

A spec is working state, not canon history. Reuse the existing spec for the same
effort, keep `docs/specs/` flat, and delete the spec once implementation,
review, validation, doc alignment, and sync are complete.

When creating or revising a spec, use
`references/task-spec-template.md` as the compact template. Keep the spec
decision-complete rather than historical; another agent should be able to resume
from the repo plus the spec without rereading chat transcripts.

## Decision Note Escape Hatch

`docs/decisions.md` is not a normal ABB docs surface. It is a narrow journal for
accepted ABB rationale that should survive active spec cleanup. Do not create or
append to it for ordinary plans, active implementation state, fallback policy,
canonical vocabulary, deferred ideas, normal PR rationale, release notes,
historical recap, or personal learning.

Use the escape hatch only for accepted rationale with no better home after the
decision passes the Decision Capture Test. Prefer the existing homes first:
`docs/specs/<task>.md`, `docs/fallbacks.md`, `docs/system-map.md`,
`docs/ubiquitous-language.md`, a GitHub issue, release notes, or changelog.

Before proposing a decision note, answer these in chat:

- Which accepted decision would be expensive or confusing to rediscover later?
- Why is the rationale not better housed in an existing ABB surface?
- What future agent or human behavior will the entry change?

If `docs/decisions.md` does not exist, propose the first entry in chat and only
create it when the user explicitly asks for that doc. If it exists, append one
compact accepted decision entry, newest first. Do not create an ADR directory,
decision-log workflow, rejected-decision archive, or historical narrative.

A completed roadmap may contribute one entry only when the durable "why" would
otherwise be lost after the roadmap/spec is deleted or goes stale. Do not keep a
whole roadmap alive merely as history.

Use this shape:

```md
## DEC-YYYY-MM-DD-short-title

Date: YYYY-MM-DD
Scope: Product intent | UI state | IPC contract | Backend lifecycle | Artifact truth

Decision:
One or two sentences stating the accepted choice.

Why:
The concise rationale that future agents need.

Alternatives:
- Include only alternatives that prevent likely re-litigation or explain a real
  trade-off.

Consequences:
What this makes easier, harder, or intentionally out of scope.

Evidence:
- PR, issue, code path, test, or canon doc that proves the decision is real.

Revisit Trigger:
The concrete condition that should make ABB reconsider the decision.
```

Do not include `Status: Accepted`; entries in `docs/decisions.md` are accepted
by definition. Do not add rejected or discarded decisions as standalone entries.
If creating or updating `docs/decisions.md`, keep the file short and run
`bash scripts/check-context-surface.sh`.

## Output Shape

For short alignment loops, answer in prose with the next question.

For substantial decisions, end with:

- **Aligned Decision**: the accepted choice or current best recommendation.
- **Why It Holds**: the domain, architecture, or product reason.
- **Boundary / Layer**: the ABB layer and owning files/docs.
- **Proof Path**: tests, checks, review evidence, or artifacts needed.
- **Routing**: no doc change, proposed issue/spec, or proposed doc patch.
- **Next Question**: one remaining high-leverage question, if any.

If the conversation exposes a surprising architecture trap, apply the repo
Canary rule from `AGENTS.md`: name the trap, the affected boundary, the immediate
assumption used to continue, and the smallest doc change that would prevent
recurrence.
