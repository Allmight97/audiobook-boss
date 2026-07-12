---
name: abb-library-research
description: Resolve ABB external-library/API uncertainty for Effect, Svelte, Tauri, Specta, or tauri-specta when implementation, IPC contract, codegen, installed-version, or upstream-source behavior affects the work. Use route cards, Context7, lockfiles, and repos/* for the active question only.
---

# ABB Library Research

Answer external-library questions that affect an ABB implementation or contract decision. Smallest source-backed answer — not a survey.

## Boundary

Vendored `repos/*`, route cards, version-sensitive API semantics, installed import truth.

Does not own metadata, path safety, job lifecycle, IPC guardrails, or release.

Skip ABB-owned domains such as metadata, processing, output artifacts, audio
engine behavior, path safety, job lifecycle, IPC guardrails, or release unless
external-library truth is the active blocker. No durable artifacts.

## Answer

- **Behavior** for the active question
- **ABB route** (owning boundary)
- **Constraints** that change implementation
- **Version or path** when version-sensitive
- **Residual uncertainty** when evidence conflicts

Version-sensitive answers must cite ABB's installed version or say the version
could not be proven.

## Source Order

1. Nearest `AGENTS.md`, code/tests, manifests, lockfiles, generated bindings,
   and installed dependencies
2. `references/<library>.md` route card
3. Current primary documentation for the installed major/minor version
4. Context7 through exactly one deterministic route
5. `repos/*` only when the answer needs implementation source, upstream tests,
   codegen behavior, runtime internals, or a docs/version mismatch check

Prefer maintainers' documentation, generated API references, source, and tests.
Use third-party explanations only to find a primary source or when no primary
source answers the question; label that limitation.

## Context7 Route

1. If a Context7 MCP is callable in the active task, use it.
2. Otherwise use the shared Context7 launcher configured for the current client.
3. If neither route is available, report that constraint and continue with
   installed truth and primary sources; do not silently substitute a broad web
   survey.

Use one known library ID and one focused query. Do not run both MCP and launcher
paths for the same question merely to accumulate evidence.

| Need | ID |
| --- | --- |
| Svelte 5 | `/sveltejs/svelte` or `/websites/svelte_dev` |
| Tauri 2 | `/websites/v2_tauri_app` |
| Effect | `/llmstxt/effect_website_llms_txt` or `/effect-ts/effect` |
| Tauri shell | `/tauri-apps/tauri-plugin-shell` |

## Completion Gate

Finish only when the answer:

- resolves the active behavior or contract question directly
- identifies ABB's installed version or explicitly says it could not be proven
- routes the implication to the owning ABB boundary
- cites the load-bearing primary or installed source
- distinguishes confirmed behavior from inference
- states residual uncertainty or the next narrow proof when evidence conflicts
- remains in chat; no note, route card, subtree refresh, or other durable
  artifact is created unless separately requested

## References

`references/subtree-management.md` for subtree refresh when requested.

Route cards: `effect.md`, `svelte.md`, `tauri.md`, `tauri-plugins.md`, `specta.md`, `tauri-specta.md`.

Pattern files: `references/pattern-<library>-<topic>.md` only after repeated need.

## Guardrails

- Installed exported types over `repos/*` copies
- No imports from `repos/*` into app code
- `src/lib/tauri/*` is the Tauri IPC boundary
- Effect stays private to owning workflows until boundary changes
