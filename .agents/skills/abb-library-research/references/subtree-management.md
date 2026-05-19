# Reference Subtree Management

Use this reference when refreshing, adding, removing, or reshaping ABB's
vendored reference subtrees and their route cards.

## Control-Plane Rule

`abb-library-research` owns the reference-library workflow:

- raw source lives at stable `repos/<library>` subtree prefixes
- route cards live in this skill's `references/<library>.md`
- task-specific pattern files, if needed, live as
  `references/pattern-<library>-<topic>.md`
- `AGENTS.md` files keep only invariants and trigger pointers

Do not create a second routing system under `repos/`, `docs/reference/`, or a
repo-local ticket ledger.

## Current Subtrees

```bash
git subtree pull --prefix=repos/effect https://github.com/Effect-TS/effect.git main --squash
git subtree pull --prefix=repos/svelte https://github.com/sveltejs/svelte.git main --squash
git subtree pull --prefix=repos/tauri https://github.com/tauri-apps/tauri.git dev --squash
git subtree pull --prefix=repos/tauri-plugins https://github.com/tauri-apps/plugins-workspace.git v2 --squash
git subtree pull --prefix=repos/specta https://github.com/specta-rs/specta.git main --squash
git subtree pull --prefix=repos/tauri-specta https://github.com/specta-rs/tauri-specta.git main --squash
```

## Refresh Verification

After a pure reference-source refresh:

- verify expected source directories still exist
- update affected route cards only if high-value entry paths changed
- verify `git status`
- do not run `scripts/checks.sh standard` unless app code, manifests,
  build/test semantics, runtime imports, or generated bindings changed

Run `bash scripts/check-context-surface.sh` when active guidance, skill text, or
route references change.

## Adding A Reference Subtree

Add a subtree only when ABB work has recurring source-level uncertainty that
official docs and installed dependency declarations do not answer cheaply.

For a new subtree:

1. choose a stable `repos/<library>` prefix
2. add the subtree with `--squash`
3. create `references/<library>.md`
4. update `SKILL.md` route selection and `repos/README.md`
5. verify source presence and context surface coherence

## Pattern Files

Do not create pattern files speculatively. Create one only after a real ABB task
shows repeated lookup friction for the same external-library idiom.

Pattern files must:

- live one level deep under `references/`
- be named `pattern-<library>-<topic>.md`
- cite concrete upstream source, test, or docs paths
- describe practical ABB usage, not broad upstream documentation
- include avoid-notes when they prevent likely misuse
- be refreshed or deleted when cited source paths stop matching subtree truth
