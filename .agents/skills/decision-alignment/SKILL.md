---
name: decision-alignment
description: ABB-specific decision alignment and planning pressure-test. Use when discussing Audiobook Boss architecture, product behavior, fallback/compat policy, domain language, repo boundaries, implementation plans, or trade-offs that may need to become durable docs, specs, issues, or code. Inspect repo artifacts before asking for known facts, ask one high-leverage question at a time, and route only accepted decisions that are hard to reverse, surprising without context, or the result of a real trade-off.
---

# Decision Alignment

## Overview

Use this skill to turn an ABB design conversation into an aligned decision,
bounded non-decision, or concrete next proof step. It is a repo-grounded
conversation mode, not a generic plan generator and not an ADR system.

Default to a proposed patch or decision summary. Do not edit durable repo docs
unless the user explicitly asks to write the change.

## Core Loop

1. Name the decision under discussion in one sentence.
2. Locate the affected ABB layer using `docs/system-map.md`:
   Product intent, UI state, IPC contract, Backend lifecycle, or Artifact truth.
3. Inspect existing repo artifacts before asking the user to restate facts:
   start with `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`,
   `docs/fallbacks.md`, and owning code/docs for the touched boundary.
4. Ask one high-leverage question at a time.
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
| Accepted durable rationale with no better home | propose `docs/decisions.md`, but do not create it by default |
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

## Optional Decisions Surface

If an accepted decision has no good existing home, propose a compact
`docs/decisions.md` entry or the creation of that file. Do this only after the
decision passes the capture test.

Use this shape when proposing an entry:

```md
## DEC-001: Short Decision Title

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
```

Do not include `Status: Accepted`; entries in `docs/decisions.md` are accepted
by definition. Do not add rejected or discarded decisions as standalone entries.

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
