# Active Spec Template

Use only when the user explicitly wants a repo-local active spec instead of a GitHub issue.

Default capture: `references/issue-template.md`.

Active specs are temporary (`docs/specs/<task>.md`). Delete or distill into canon when done.

```md
# <Short outcome title>

Status: temporary active spec.

## Owning invariant

> One sentence: what truth this work enforces everywhere.

## Current state

- What is true in `main` today (file-path evidence when load-bearing)
- Where the invariant currently fails, if it does

## Scope

In scope:

- ...

Out of scope:

- ...

## Plan

Numbered steps with dependency order when steps must serialize.

## Verification

- Targeted commands and tests
- Manual or visual checks when static tests are insufficient

## Open forks

- Fork A vs Fork B — default: <which one> (only when product choice remains)

Or omit when the decision is locked.

## Cleanup

When implemented, rejected, or superseded: delete this spec or distill enduring rules into canon (`AGENTS.md`, `docs/system-map.md`, `docs/ubiquitous-language.md`, `docs/api-map.md`, `docs/DECISIONS.md`).
```