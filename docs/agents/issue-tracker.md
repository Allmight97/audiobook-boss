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

Create a GitHub issue. Read `docs/agents/triage-labels.md` for label strings.

## Fetch a ticket

Run `gh issue view <number> --comments`.

## Issue body rules

Issues are durable work surfaces, not planning transcripts. Bodies must be resume-ready without chat context.

Include: owning invariant, current state, plan, verification path, open forks (if any).

Omit: restructure dates, skill names, scout provenance, "verified via research", superseded-framing appendices, and source-ladder narration.

Use `docs/ubiquitous-language.md` vocabulary. Prefer module and seam names over file paths unless a path is load-bearing for verification.

See `.agents/skills/decision-alignment/references/issue-template.md` for the default ABB engineering issue shape.