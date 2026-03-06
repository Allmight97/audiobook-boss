# Documentation Directives

## Scope

- Applies to Markdown and documentation routing under `docs/`.
- Keeps the repo knowledge plane agent-legible and explicit about canonical versus historical material.

## Preferred Path

- Start in `docs/README.md` before editing or citing deeper docs.
- Treat `docs/specs/technical-reference.md` as the architecture/runtime source of truth.
- Treat `docs/verification.md` as the source of truth for change-type verification expectations.
- Treat `docs/decisions/DECISIONS.md` plus individual ADRs as the durable decision log.
- Use `docs/external-apis/README.md` for boundary/reference drill-down, not as the top-level docs entrypoint.

## Hard Invariants

- Canonical docs must describe current repo truth, not historical branch-local state.
- Historical trackers and closure plans must be marked non-canonical at the top of the file.
- If a doc contradicts code or a canonical doc, update or demote the doc in the same change; do not leave stale guidance behind.
- Commands, file paths, and skills mentioned in docs must resolve in the current repo.
- Avoid inventing parallel “summary” docs when an existing canonical doc can be tightened instead.

## Done Criteria

- Added/edited docs are reachable from `docs/README.md` or intentionally marked historical/reference-only.
- Canonical docs point to real commands, files, and owner surfaces.
- Historical docs no longer read like active policy or current execution plans.
