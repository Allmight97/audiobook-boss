# Issue tracker: GitHub

Issues for this repo live as GitHub issues on `Allmight97/audiobook-boss`. Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`
- **List issues**: `gh issue list --state open --json number,title,body,labels`
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

Infer the repo from `git remote -v` — `gh` does this automatically when run inside a clone.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## Publish durable work

Draft the issue in chat by default. Create, edit, comment on, label, or close a
GitHub issue only when the user explicitly authorizes that scoped external
mutation. Agreement on the issue content does not itself authorize publication.

Read `docs/agents/triage-labels.md` for label strings and the
`ready-for-agent` gate.

## Fetch a ticket

Run `gh issue view <number> --comments`.

## Issue body rules

Issues are durable work surfaces, not planning transcripts. Bodies must be resume-ready without chat context.

Use this shape for substantial engineering work:

```md
## Owning invariant

> One sentence: what truth this work enforces everywhere.

## Current state

- What is true in `main` today, with file-path evidence when load-bearing
- Where the invariant currently fails, if it does

## Plan

Numbered steps, ordered when dependencies must serialize.

## Verification

- Targeted commands and tests
- Manual or visual checks when static tests are insufficient

## Open forks

- Fork A vs Fork B — default: <which one>
```

Omit `Open forks` when the decision is locked. Include library versions only
when they change implementation choices.

Omit: restructure dates, skill names, scout provenance, "verified via research", superseded-framing appendices, and source-ladder narration.

Use `docs/ubiquitous-language.md` vocabulary. Prefer module and seam names over file paths unless a path is load-bearing for verification.

For large work that needs vertical slices, publish the parent issue first. Use
`to-issues` only when the user asks for the breakdown.
