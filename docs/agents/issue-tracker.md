# Issue Tracker: GitHub

Issues for this repo live on `Allmight97/audiobook-boss`. Use the authenticated
GitHub connector when the agent has it; otherwise use `gh issue` (create, view
with `--comments`, list, comment, edit, close). Work requests arrive as issues.

## Publish durable work

Draft the issue in chat by default. Create, edit, comment on, label, or close a
GitHub issue only when the user explicitly authorizes that scoped external
mutation. Agreement on the issue content does not itself authorize publication.

Label strings and the `ready-for-agent` gate: `docs/agents/triage-labels.md`.

## Issue body rules

Issues are durable work surfaces. A body is resume-ready without chat context
and states current truth and the next action. Process history stays out.

Use this shape for substantial engineering work:

```md
## Current state and next action

- What is true in `main` today, with file-path evidence when load-bearing
- Where the invariant currently fails, if it does
- The single next action or decision

## Owning invariant

> One sentence: what truth this work enforces everywhere.

## Plan

Numbered steps, ordered when dependencies must serialize.

## Verification

- Targeted commands and tests
- Manual or visual checks when static tests are insufficient

## Open forks

- Fork A vs Fork B — default: <which one>
```

Omit `Open forks` when the decision is locked. Include library versions only
when they change implementation choices. An issue that asks the owner to
choose what the user gets cites `docs/agents/observed-use.md` or says "no
observed case" (`.agents/skills/app-use-notes`).

When work lands, rewrite the issue around the resulting state or close it with
the proof and residual work. A closed flag does not make a stale body safe to
follow, and an open issue keeps no next action that already happened.

Use terminology from the owning interface and nearest `AGENTS.md`. Prefer
module and seam names over file paths unless a path is load-bearing for
verification.

For large work that needs vertical slices, publish the parent issue first. Use
`to-issues` only when the user asks for the breakdown.
