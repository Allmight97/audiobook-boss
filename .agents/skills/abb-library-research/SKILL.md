---
name: abb-library-research
description: ABB-specific library/docs and reference-source router. Use during Audiobook Boss planning, implementation, review, or audit when external API behavior, Context7 docs, vendored repos/* source, installed dependency surfaces, or Effect/Svelte/Tauri/Specta/tauri-specta behavior affects the work.
---

# ABB Library Research

Use this skill when ABB work depends on external library behavior or source
patterns. Optimize for implementation-ready answers, not broad research.

## Posture

ABB is current-state evidence and the owning integration surface, not the final
authority on what should be built. Use ABB to locate boundaries, constraints,
tests, and product intent; use well-regarded libraries, current docs, vendored
source, and installed dependency truth to challenge, improve, or validate the
plan.

This is a supporting router for planning and implementation agents. It grounds
work in real sources without absorbing domain skills such as metadata, path
safety, job lifecycle, release, dependency maintenance, or IPC guardrails.

## Start Here

1. Name the ABB owner surface and dependency gate first: `AGENTS.md`, nearest
   nested `AGENTS.md`, relevant docs/source/tests, `package.json`, lockfiles,
   generated bindings, and active capability/config files.
2. Use Context7 for fast public-doc discovery, examples, and planning context.
3. Use `repos/*` for source-level confirmation, tests, examples, permission
   schemas, and implementation patterns.
4. Reconcile against ABB's installed surface before importing or mirroring
   anything:
   `package.json`, `bun.lock`, `node_modules` type declarations, Cargo
   lockfiles, generated schemas, or generated bindings.
5. Use current official docs, release notes, changelogs, advisories, or
   maintainer repos when the claim is version-sensitive, security-sensitive, or
   likely to have changed.

If Context7 or web tools are degraded, keep moving with official sites,
vendored source, installed declarations, local generated docs, and `rg`.

## Planning Mode

For implementation planning, produce the smallest source-backed decision:

- library/API behavior that matters
- recommended ABB route
- constraints or gotchas
- source ladder used
- targeted acceptance checks

Do not produce broad library surveys unless the user explicitly asks for one.

## Implementation And Review Mode

- Prefer exported installed dependency types over copying conditional types or
  local mirrors from reference repos.
- Use `repos/*` as read-only source material; do not import from it.
- Keep `src/lib/tauri/*` as ABB's runtime boundary for Tauri IPC/plugin access.
- For Effect pilots, keep Effect private to the owning workflow until the
  boundary decision changes intentionally.
- For stale or conflicting evidence, name the mismatch and let installed ABB
  dependencies decide importable API truth.
- Absence from ABB dependencies is an adoption/planning fact, not proof that the
  library is irrelevant.

## Route Map

- Effect workflow, typed errors, services, layers, scopes, streams, schedules,
  tests: Context7 `/effect-ts/effect` or effect.website docs, then
  `repos/effect/packages/effect`, then ABB dependency state.
- Svelte 5 components, runes, events, stores, compiler/runtime behavior:
  Context7 Svelte docs, then `repos/svelte/packages/svelte` and targeted tests,
  then installed `svelte` declarations/version.
- Tauri core JS/Rust APIs, commands, events, runtime behavior:
  Context7 Tauri docs, then `repos/tauri/packages/api`,
  `repos/tauri/crates/tauri`, and `repos/tauri/examples`, then installed
  `@tauri-apps/*` or Cargo surfaces.
- Tauri plugins: Context7 Tauri plugins docs, then
  `repos/tauri-plugins/plugins/<plugin-name>` and examples, then installed
  plugin declarations and `src-tauri/capabilities`.
- Specta type export behavior: Context7/current docs if available, then
  `repos/specta/specta`, `repos/specta/specta-typescript`, tests/examples, and
  generated ABB bindings.
- Tauri/Specta integration: Context7/current docs if available, then
  `repos/tauri-specta` source/tests/examples, then ABB generated binding checks.

## Subtree Refreshes

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
`git status`. Do not run `scripts/checks.sh standard` for pure reference-source
refreshes.
