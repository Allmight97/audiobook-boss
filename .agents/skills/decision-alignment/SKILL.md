---
name: decision-alignment
description: ABB repo-grounded alignment when substantial work needs a locked decision or GitHub issue capture. Inspect repo truth first; at most two action-changing questions. Default capture is GitHub issues.
---

# Decision Alignment

Turn an ABB conversation into an aligned decision and the smallest durable capture.

Propose a decision and verification route. Edit durable repo docs only when the user asks.

## Boundary

ABB work that depends on repo truth, domain language, code boundaries, or durable capture.

Generic pressure-testing without ABB capture → `grill-me`.

Other workflows (architecture scan, library lookup, handoff, issue slicing) start only when the user invokes them.

## Core Loop

1. State the outcome in one sentence; use `docs/ubiquitous-language.md` when terms exist.
2. Verify the affected layer in `docs/system-map.md`.
3. Inspect before asking: `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`, owning code/tests.
4. At most two action-changing questions with recommended answers.
5. Stop when the decision is locked, open forks have a default, or issue capture is ready.

## Challenge Rules

- Replace fuzzy language with `docs/ubiquitous-language.md` terms.
- Name actor, input, boundary, terminal outcome, falsifying evidence.
- Cross-check claims in code, tests, canon docs.
- Name contradictions; ask which source should change.

## Capture

Default: GitHub issue per `docs/agents/issue-tracker.md`. Load `references/issue-template.md` when publishing.

Draft in chat by default. Create or label the GitHub issue only when the user
explicitly authorizes that external mutation.

Label `ready-for-agent` per `docs/agents/triage-labels.md` only after the gate
below passes.

Session handoff → `handoff` to OS temp. Do not write handoff files in the repo
or duplicate an issue body into a handoff.

| Outcome | Home |
| --- | --- |
| Substantial engineering work | GitHub issue |
| Vertical slice breakdown | `to-issues` (user invokes) |
| Provider degradation | Owning command, `docs/api-map.md`, tests |
| Tag compatibility | `audiobook-metadata` |
| Ownership shape | `docs/system-map.md` |
| Term fix | `docs/ubiquitous-language.md` |
| Durable rationale, no better home | `docs/DECISIONS.md` |
| Personal learning | Outside repo |

`docs/specs/<task>.md` only when the user explicitly wants a repo-local spec. Load `references/active-spec-template.md`; delete or distill when done.

`docs/DECISIONS.md`: accepted, durable, non-obvious rationale only. Follow the
current file's compact shape; edit only when the user asks. Do not introduce a
parallel ADR or `CONTEXT.md` system.

## Ready-for-Agent Gate

Apply `ready-for-agent` only when a fresh agent can act without chat context:

- current `main` truth and the affected owner are explicit
- the owning invariant and terminal outcome are unambiguous
- scope and ordered dependencies are stated
- proof is located at the owner seam, including manual evidence where needed
- no unresolved human decision remains; any open implementation fork has an
  explicit default and escalation trigger
- the body is resume-ready and contains no hidden dependency on conversation
- publication or other external mutation still requires explicit authority

If the gate fails, keep the issue unlabelled and name the missing input.

## Output Shape

Short loop: prose + next question.

Substantial alignment:

- **Aligned Decision**
- **Why It Holds**
- **Boundary / Layer**
- **Verification Path**
- **Capture**
- **Next Question** (if any)

Issue bodies follow `references/issue-template.md` strip rules. Active specs use
`references/active-spec-template.md` only when explicitly requested.
