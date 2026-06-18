# Active Spec Template

Use this template with `decision-alignment` only when the user explicitly wants a repo-local active spec instead of a GitHub issue.

Default durable capture for substantial ABB work is GitHub issues (`references/issue-template.md`).

Active specs must be self-contained, current, outcome-verifiable, and temporary (stored in `docs/specs/<task>.md`).
When work is done, delete the spec or distill enduring truths into canon.

```md
# <Task Or Feature> — Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome:
Acceptance signal:

## Progress

- [ ] YYYY-MM-DD: ...

## Surprises & Discoveries

- Observation:
  Evidence:

## Decision Log

- Decision:
  Rationale:
  Date:

## Context And Orientation

- Current repo state checked.
- Owning boundaries and files.
- Terms from `docs/ubiquitous-language.md` that matter.
- Canon surfaces this spec must not redefine.

## Scope And Constraints

In scope (Attractors):

- ...

Out of scope (Repellors):

- ...

Constraints:

- ...

## Plan Of Work

- Edits:
- Verification steps:
- Expected repo-visible outcome:

## Interfaces And Dependencies

- Modules/commands/types:
- Libraries/external behavior:
- Dependency constraints:

## Verification Path and Checks

- Targeted checks:
- Full review, if needed:
- Manual or visual evidence, if needed:

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec, and consider distilling (if any) enduring high-ROI elements into existing (or new) canon:
  - `AGENTS.md` / nearest nested `AGENTS.md`
  - `docs/system-map.md`
  - `docs/ubiquitous-language.md`
  - `docs/api-map.md`
  - README/changelog/release notes (using release skills)
  - GitHub issue for deferred work (as aligned and agreed upon with repo admins/orchestrators)
```
