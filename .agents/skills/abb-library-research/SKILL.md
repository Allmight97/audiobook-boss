---
name: abb-library-research
description: Resolve ABB external library/API uncertainty for Effect, Svelte, Tauri, Specta, and tauri-specta via route cards, repos/*, Context7, and installed deps. Facts for the active question only.
---

# ABB Library Research

Answer external-library questions that affect an ABB implementation or contract decision. Smallest source-backed answer — not a survey.

## Boundary

Vendored `repos/*`, route cards, version-sensitive API semantics, installed import truth.

Does not own metadata, path safety, job lifecycle, IPC guardrails, or release.

User invokes when library truth blocks the answer. No durable artifacts.

## Answer

- **Behavior** for the active question
- **ABB route** (owning boundary)
- **Constraints** that change implementation
- **Version or path** when version-sensitive
- **Residual uncertainty** when evidence conflicts

## Lookup order

1. Nearest `AGENTS.md`, code/tests, manifests, lockfiles, generated bindings
2. `references/<library>.md` route card
3. Context7 (one path: CLI or MCP)
4. `repos/*`
5. Installed truth in `package.json`, `bun.lock`, Cargo lockfiles

Context7: known library ID + one query. No broad surveys unless asked.

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