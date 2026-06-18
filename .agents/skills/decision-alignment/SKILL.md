---
name: decision-alignment
description: ABB repo-grounded alignment when substantial Audiobook Boss work needs a locked decision, GitHub issue capture, or canon doc route. Inspect repo truth first; ask at most two action-changing questions. Default durable capture is GitHub issues, not docs/specs. Do not auto-run architecture scans or library research; do not chain other planning skills.
---

# Decision Alignment

Turn an ABB design, architecture, or product conversation into an aligned decision and the smallest durable capture.

Default to a proposed decision and verification route. Do not edit durable repo docs unless the user asks.

## Boundary

Use for ABB work that depends on current repo truth, domain language, code boundaries, or durable capture.

Do not use for generic pressure-testing without ABB routing — use global `grill-me`.

Do not invoke `improve-codebase-architecture`, `abb-library-research`, `handoff`, or `to-issues` automatically. The user starts those separately when needed.

## Core Loop

1. State the outcome in one sentence using `docs/ubiquitous-language.md` terms when they exist.
2. Verify the affected layer in `docs/system-map.md`: Product intent, UI state, IPC contract, Backend lifecycle, or Artifact truth.
3. Inspect before asking: `AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`, owning code/tests for the touched boundary.
4. Ask at most two action-changing questions with recommended answers. Lists of facts count as questions.
5. Stop when the decision is locked, open forks are explicit with a default, or capture to a GitHub issue is ready.

## Challenge Rules

- Replace fuzzy language with terms from `docs/ubiquitous-language.md`.
- Name actor, input, boundary crossed, terminal outcome, and falsifying evidence for concrete scenarios.
- Cross-check claims in owning code, tests, or canon docs.
- Name contradictions between code, docs, tests, or conversation; ask which source should change.

## Capture

**Default home for substantial work:** GitHub issue per `docs/agents/issue-tracker.md`.

When publishing an issue, load `references/issue-template.md` and apply strip rules.

Apply `ready-for-agent` from `docs/agents/triage-labels.md` when the issue is complete without chat context.

For session handoff after alignment, point the user at global `handoff` and `references/handoff-template.md` — do not write handoff files into the repo.

### Other homes

| Outcome | Home |
| --- | --- |
| Deferred or substantial engineering work | GitHub issue (default) |
| Vertical slice breakdown of approved plan | User invokes global `to-issues` |
| External provider degradation | Owning command module, `docs/api-map.md`, focused tests |
| External file/tag compatibility | `.agents/skills/audiobook-metadata` |
| Stable ownership shape | `docs/system-map.md` |
| Canonical term fix | `docs/ubiquitous-language.md` |
| Accepted rationale, no better home | `docs/DECISIONS.md` escape hatch |
| Personal learning | Outside repo |
| Transient reasoning | Nowhere durable |

### Active spec (exception)

Use `docs/specs/<task>.md` only when the user explicitly wants a repo-local spec instead of an issue. When used, load `references/active-spec-template.md`. Delete or distill when done.

## Decision Note Escape Hatch

Use `docs/DECISIONS.md` only for accepted rationale that should survive issue cleanup. Not a changelog, PR recap, or process log.

Load `references/decision-note-template.md` when appending. Create or edit only when the user asks.

## Output Shape

**Short loop:** prose + next question.

**Substantial alignment:**

- **Aligned Decision** (or recommendation with default)
- **Why It Holds**
- **Boundary / Layer**
- **Verification Path**
- **Capture:** proposed or published GitHub issue; or explicit no capture
- **Next Question** (if any)

## Narration ban

Durable output must not document how it was produced. No skill names, scout framing, restructure dates, or superseded appendices in issues or specs.