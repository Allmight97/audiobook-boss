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

## Current Reference Map

- `repos/effect`: Effect services, layers, streams, scopes, schedules, typed
  errors, test utilities, and idiomatic Effect examples.
- `repos/svelte`: Svelte 5 runtime/compiler behavior and component patterns.
- `repos/tauri`: Tauri runtime, command, event, API, plugin, and bundling
  behavior.
- `repos/tauri-plugins`: Tauri v2 plugin source, including dialog and opener
  behavior used by ABB's runtime boundary.
- `repos/specta`: Rust type export and TypeScript binding generation behavior.
- `repos/tauri-specta`: Tauri/Specta integration and event/command export
  behavior.

## Refresh Pattern

Refresh only as an explicit dependency/reference maintenance task:

```bash
git subtree pull --prefix=repos/effect https://github.com/Effect-TS/effect.git main --squash
```

Use the upstream branch recorded for each subtree. Keep refreshes grouped by
coherent stack area. Refreshing or adding reference subtrees does not require
`scripts/checks.sh standard` unless ABB app code, dependency manifests,
build/test semantics, or runtime imports also change.
