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

## Lookup order

1. Nearest `AGENTS.md`, code/tests, manifests, lockfiles, generated bindings, installed dependencies
2. `references/<library>.md` route card
3. Context7 through the active MCP tool; if unavailable, use the shared local
   launcher `/Users/jstar/.agents/bin/context7-mcp` through client MCP config.
4. `repos/*` only when the answer needs implementation source, upstream tests,
   codegen behavior, runtime internals, or Context7/doc version mismatch checks

Context7: known library ID + one query through the active MCP tool or shared
launcher. Do not add per-agent Context7 plugins, run `ctx7 setup`, or create
separate API-key routing when the shared launcher is available. No broad
surveys unless asked.

| Need | ID |
| --- | --- |
| Svelte 5 | `/sveltejs/svelte` or `/websites/svelte_dev` |
| Tauri 2 | `/websites/v2_tauri_app` |
| Effect | `/llmstxt/effect_website_llms_txt` or `/effect-ts/effect` |
| Tauri shell | `/tauri-apps/tauri-plugin-shell` |

## References

`references/subtree-management.md` for subtree refresh when requested.

Route cards: `effect.md`, `svelte.md`, `tauri.md`, `tauri-plugins.md`, `specta.md`, `tauri-specta.md`.

Pattern files: `references/pattern-<library>-<topic>.md` only after repeated need.

## Guardrails

- Installed exported types over `repos/*` copies
- No imports from `repos/*` into app code
- `src/lib/tauri/*` is the Tauri IPC boundary
- Effect stays private to owning workflows until boundary changes
