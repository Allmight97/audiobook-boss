# ABB Issue Template

Use when publishing substantial engineering work to GitHub. Load at capture time only.

Read `docs/agents/issue-tracker.md` for `gh` conventions and `docs/agents/triage-labels.md` for labels.

## Strip rules (mandatory)

Omit from the issue body:

- Dates, "restructured", "supersedes", or appendix of old framings
- Skill names, scout/candidate numbering, or research methodology
- "Verified via", "grounded in", "source ladder used"
- Chat transcript, progress logs, or decision history

Include library versions only when they change implementation choices.

## Body shape

```md
# <Short outcome title>

## Owning invariant

> One sentence: what truth this work enforces everywhere.

## Current state

- What is true in `main` today (file-path evidence when load-bearing)
- Where the invariant currently fails, if it does

## Plan

Numbered steps with dependency order when steps must serialize.

## Verification

- Targeted commands and tests
- Manual or visual checks when static tests are insufficient

## Open forks

- Fork A vs Fork B — default: <which one> (only when product choice remains)

Or omit this section when the decision is locked.
```

Apply `ready-for-agent` when the issue is complete enough for an agent without chat context.

For large work the user wants sliced, stop after the parent issue and let the user invoke global `to-issues`.