# Vendored Reference Repositories

`repos/*` contains squashed git subtrees of external projects that ABB agents use
as local reference source. These directories are not application code and are not
dependency import targets.

Use `.agents/skills/abb-library-research` as the control plane before searching
a subtree. Its `references/` files are the route cards that tell agents where to
look, which examples or tests are worth reading, and what ABB-local dependency
state still needs to be checked.

## Stable Subtree Prefixes

Keep raw subtrees at their current prefixes:

- `repos/effect`
- `repos/svelte`
- `repos/tauri`
- `repos/tauri-plugins`
- `repos/specta`
- `repos/tauri-specta`

Those prefixes are part of the documented `git subtree pull --prefix=...`
refresh commands. Do not move or nest the raw subtrees to improve navigation.
Keep agent routing, notes, and future task-specific pattern files inside
`.agents/skills/abb-library-research/references/`.

## Authority

Use route cards for navigation only. For implementation decisions, reconcile:

1. ABB owner surfaces and dependency manifests.
2. Current docs when behavior is version-sensitive.
3. Raw `repos/*` source, examples, and tests.
4. ABB installed dependencies, generated bindings, and Cargo surfaces.
