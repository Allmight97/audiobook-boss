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

Label `ready-for-agent` per `docs/agents/triage-labels.md` when complete without chat.

Session handoff → `handoff` to OS temp; see `references/handoff-template.md`. Do not write handoff files in the repo.

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

`docs/DECISIONS.md`: accepted rationale only. Load `references/decision-note-template.md`; edit when the user asks.

## Output Shape

Short loop: prose + next question.

Substantial alignment:

- **Aligned Decision**
- **Why It Holds**
- **Boundary / Layer**
- **Verification Path**
- **Capture**
- **Next Question** (if any)

Issue and spec bodies follow `references/issue-template.md` strip rules.