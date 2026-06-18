---
name: abb-library-research
description: Resolve ABB-specific external library and API uncertainty for Effect, Svelte, Tauri, Specta, and tauri-specta using route cards, repos/* reference source, Context7, and installed dependency truth. Return facts for the active decision only. Do not publish issues, specs, or planning artifacts.
---

# ABB Library Research

Answer external-library questions that affect an ABB implementation or contract decision. Produce the smallest source-backed answer — not a survey.

## Boundary

Use when ABB work depends on vendored `repos/*` behavior, route cards, version-sensitive API semantics, or installed import truth.

Do not absorb metadata, path safety, job lifecycle, IPC guardrails, or release workflows.

Do not run during alignment unless library truth is the blocking unknown. Do not chain into issue capture or architecture scans.

## Answer contract

Return only:

- **Behavior** that matters for the active question
- **ABB route** (which boundary owns the change)
- **Constraints / gotchas** that change implementation
- **Version or path evidence** when version-sensitive (package version, generated binding path, or `repos/*` path)
- **Residual uncertainty** when evidence conflicts — name the mismatch; let installed deps decide importable truth

Do not return "source ladder used", "verified via research", or methodology narration unless the user asked why evidence is uncertain.

## Source order

1. Nearest `AGENTS.md`, owning code/tests, manifests, lockfiles, generated bindings
2. Relevant `references/<library>.md` route card
3. Context7 (CLI or MCP — one path per lookup, not both)
4. `repos/*` for source-level patterns and tests
5. Reconcile with installed/importable truth in `package.json`, `bun.lock`, Cargo lockfiles

Context7: known library ID + one precise query. Skip resolve when ID is known. No broad surveys unless the user asks.

ABB library IDs:

| Need | Prefer |
| --- | --- |
| Svelte 5 | `/sveltejs/svelte` or `/websites/svelte_dev` |
| Tauri 2 | `/websites/v2_tauri_app` |
| Effect | `/llmstxt/effect_website_llms_txt` or `/effect-ts/effect` |
| Tauri shell plugin | `/tauri-apps/tauri-plugin-shell` |

## References

- `references/subtree-management.md` — subtree refresh only when requested
- Route cards: `references/effect.md`, `references/svelte.md`, `references/tauri.md`, `references/tauri-plugins.md`, `references/specta.md`, `references/tauri-specta.md`

Route cards navigate; they do not override installed deps or generated bindings.

## Pattern files

Create `references/pattern-<library>-<topic>.md` only after repeated need. Delete when stale.

## Implementation guardrails

- Prefer installed exported types over copying from `repos/*`
- Do not import from `repos/*` into app code
- Keep `src/lib/tauri/*` as the Tauri IPC boundary
- Keep Effect private to owning workflows until a boundary decision changes