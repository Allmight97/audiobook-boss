---
name: abb-library-research
description: ABB reference-library control plane for external library/API research, vendored repos/* subtrees, route cards, subtree refreshes, and pattern files. Use during ABB planning, implementation, review, audits, or maintenance involving Context7/current docs, installed dependency truth, or Effect/Svelte/Tauri/Specta/tauri-specta behavior.
---

# ABB Library Research

Use for ABB work that depends on external library behavior, reference subtree
source, or route-card maintenance. Produce implementation-ready answers, not
broad research.

## Posture

Use ABB to locate boundaries, constraints, tests, and product intent. Use
well-regarded libraries, current docs, this skill's route references, `repos/*`
reference source, and installed dependency truth to challenge, improve, or
validate the plan.

This skill is the control plane for ABB reference libraries: source routing,
subtree refresh guidance, route-card maintenance, and task-specific pattern-file
policy. It does not absorb domain skills such as metadata, path safety, job
lifecycle, release, dependency maintenance, or IPC guardrails.

## References

- Read `references/subtree-management.md` when refreshing subtrees, adding or
  removing reference libraries, updating route cards, or creating pattern files.
- Read the relevant route card before searching raw source:
  `references/effect.md`, `references/svelte.md`, `references/tauri.md`,
  `references/tauri-plugins.md`, `references/specta.md`, or
  `references/tauri-specta.md`.
- Route cards are navigation only. They do not override upstream source,
  current docs, ABB manifests, generated bindings, or installed dependencies.

## Source Ladder

1. Locate ABB ownership first: nearest `AGENTS.md`, relevant docs/source/tests,
   dependency manifests, lockfiles, generated bindings, and active config.
2. Use Context7/ctx7 MCP only when external library/API behavior is uncertain
   or version-sensitive. Use known library IDs when possible, ask one precise
   question, and skip library resolution when the ID is already known.
3. Read the relevant `references/<library>.md` route card to choose focused
   source, test, example, and docs paths.
4. Confirm with `repos/*` for source-level patterns, tests, examples,
   permission schemas, and implementation details.
5. Reconcile with ABB's installed/importable truth before implementation:
   `package.json`, `bun.lock`, `node_modules` declarations, Cargo lockfiles,
   generated schemas, and generated bindings.

## Context7 Discipline

- Known library ID plus one precise query is the default.
- Do not run broad surveys or repeated resolve/query loops unless the user asks.
- Avoid deep/research mode unless ordinary docs plus `repos/*` are insufficient.
- Use `ctx7` CLI only when the user explicitly asks to diagnose Context7 tooling
  or compare MCP behavior. Do not use CLI for ordinary ABB library research.
- If Context7 is degraded, keep moving with official docs, `repos/*`, installed
  declarations, generated bindings, and `rg`.

## Pattern Files

- Do not create pattern files speculatively.
- Create `references/pattern-<library>-<topic>.md` only when a real ABB task
  repeatedly needs the same external-library idiom.
- Pattern files must cite concrete source/test/docs paths, stay practical for
  ABB usage, include what to avoid when useful, and be refreshed or deleted when
  stale.

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
- Use this skill's route references to find the right subtree paths, then use
  `repos/*` as read-only source material; do not import from it.
- Keep `src/lib/tauri/*` as ABB's runtime boundary for Tauri IPC/plugin access.
- For Effect pilots, keep Effect private to the owning workflow until the
  boundary decision changes intentionally.
- For stale or conflicting evidence, name the mismatch and let installed ABB
  dependencies decide importable API truth.
- Absence from ABB dependencies is an adoption/planning fact, not proof that the
  library is irrelevant.

## Route Map

- Effect workflow, typed errors, services, layers, scopes, streams, schedules,
  tests: Context7/ctx7 MCP Effect docs such as
  `/llmstxt/effect_website_llms_txt` for usage guidance or `/effect-ts/effect`
  for API/source examples, then `references/effect.md`, then listed
  `repos/effect` paths, then ABB dependency state.
- Svelte 5 components, runes, events, stores, compiler/runtime behavior:
  Context7/ctx7 MCP Svelte docs, then `references/svelte.md`, then listed
  `repos/svelte` paths and targeted tests, then installed `svelte`
  declarations/version.
- Tauri core JS/Rust APIs, commands, events, runtime behavior:
  Context7/ctx7 MCP Tauri docs, then `references/tauri.md`, then listed
  `repos/tauri` paths, then installed `@tauri-apps/*` or Cargo surfaces.
- Tauri plugins: Context7/ctx7 MCP Tauri plugins docs, then
  `references/tauri-plugins.md`, then listed
  `repos/tauri-plugins/plugins/<plugin-name>` paths and examples, then
  installed plugin declarations and `src-tauri/capabilities`.
- Specta type export behavior: Context7/current docs if available, then
  `references/specta.md`, then listed `repos/specta` paths, tests/examples,
  and generated ABB bindings.
- Tauri/Specta integration: Context7/current docs if available, then
  `references/tauri-specta.md`, then listed `repos/tauri-specta`
  source/tests/examples, then ABB generated binding checks.

## Subtree Refreshes (only when requested)

Read `references/subtree-management.md`, use the recorded subtree command, then
verify subtree metadata, expected source directories, and `git status`. Do not
run `scripts/proof.sh standard` for pure reference-source refreshes.
