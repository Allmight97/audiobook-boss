---
name: reference-repos
description: Navigate ABB's vendored frontend and boundary reference repositories under repos/. Use when working with Effect, Svelte, Tauri, Tauri plugins, Specta, tauri-specta, TS/Rust boundary behavior, or agent-harness source examples.
---

# Reference Repos

Use this skill when external frontend or boundary-library behavior matters for
ABB implementation, review, or planning.

## Rule

Start from ABB ownership first, then inspect the matching vendored source.

- ABB invariants live in `AGENTS.md`, `docs/system-map.md`, `docs/api-map.md`,
  nested `AGENTS.md`, and the owning source/tests.
- External implementation patterns live under `repos/*`.
- `repos/*` is read-only reference material, not application code.
- Do not import from `repos/*`; ABB code imports from normal dependencies.
- If current release behavior may have changed, combine this skill with global
  `lib-research` or current official docs.

## Validation Ladder

When a library detail affects ABB implementation or review, validate in this
order and name any mismatch:

1. Context7/current official docs for the library's intended public behavior.
2. The matching squashed reference repo under `repos/*` for source-level API
   shape, tests, permission schemas, and examples.
3. ABB's installed dependency surface (`package.json`, `bun.lock`,
   `node_modules` type declarations, Cargo lockfiles, or generated schemas) for
   what the current app can actually import.

Prefer exported library types from installed packages over copying conditional
types or mirrors from `repos/*`. Use local mirrors only when the dependency does
not export the needed type and the mirror is covered by a focused contract test.

## Route By Question

- Effect workflow, typed errors, services, layers, scopes, streams, schedules,
  or Effect tests: inspect `repos/effect/packages/effect` first, then nearby
  package tests/examples.
- Svelte 5 component, runtime, compiler, or store/rune behavior: inspect
  `repos/svelte/packages/svelte` and targeted tests under that subtree.
- Tauri JS API, events, plugins, command behavior, or app runtime details:
  inspect `repos/tauri/packages/api`, `repos/tauri/crates/tauri`, and relevant
  examples under `repos/tauri/examples`.
- Tauri plugin behavior, including ABB's dialog and opener dependencies:
  inspect `repos/tauri-plugins/plugins/<plugin-name>` first, especially
  `repos/tauri-plugins/plugins/dialog` and `repos/tauri-plugins/plugins/opener`.
- Specta type export behavior: inspect `repos/specta/specta`,
  `repos/specta/specta-typescript`, and tests/examples under `repos/specta`.
- Tauri/Specta integration behavior: inspect `repos/tauri-specta` and its
  examples.

## ABB-Specific Use

- Keep `src/lib/tauri/*` as ABB's runtime boundary; reference repos inform the
  adapter but do not replace it.
- For an Effect pilot, keep Effect private to an owning workflow such as Status
  Panel Runtime until the boundary decision changes intentionally.
- Prefer creating small `docs/specs/*` or issue-ready notes for durable ABB
  decisions; do not add pattern files unless they will prevent repeated
  rediscovery.

## Update Commands

Use subtree refreshes only when explicitly asked:

```bash
git subtree pull --prefix=repos/effect https://github.com/Effect-TS/effect.git main --squash
git subtree pull --prefix=repos/svelte https://github.com/sveltejs/svelte.git main --squash
git subtree pull --prefix=repos/tauri https://github.com/tauri-apps/tauri.git dev --squash
git subtree pull --prefix=repos/tauri-plugins https://github.com/tauri-apps/plugins-workspace.git v2 --squash
git subtree pull --prefix=repos/specta https://github.com/specta-rs/specta.git main --squash
git subtree pull --prefix=repos/tauri-specta https://github.com/specta-rs/tauri-specta.git main --squash
```

After refreshes, verify subtree metadata, expected source directories, and
`git status`. Run the context-surface check only when AGENTS/skill/docs guidance
changed:

```bash
bash scripts/check-context-surface.sh
```

Do not run `scripts/checks.sh standard` for pure reference-source refreshes.
