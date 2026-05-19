# Vendored Reference Repositories

## Scope

- `repos/*` contains squashed git subtrees of external projects used as
  agent-readable reference material.
- These repositories are not Audiobook Boss application code.
- Upstream `AGENTS.md` files inside a vendored repository describe that upstream
  project only; they do not authorize ABB application edits inside `repos/*`.

## Invariants

- Treat `repos/*` as read-only unless the user explicitly asks to refresh or
  patch a vendored reference repository.
- Do not import application code from `repos/*`; ABB code imports from normal
  package or crate dependencies.
- Do not run ABB formatters, linters, or tests against `repos/*`.
- Prefer focused `rg`, `fd`, and targeted file reads inside the relevant
  subtree over broad scans of every vendored repo.

## Control Plane

- Use `.agents/skills/abb-library-research` for route cards, subtree refresh
  commands, and task-specific pattern-file policy.
- Keep detailed routing out of `AGENTS.md`; this file only protects the raw
  subtree boundary and points agents to the owning skill.

For available subtrees, refresh commands, and route cards, use the owning skill's
`references/` files rather than duplicating routing here.
