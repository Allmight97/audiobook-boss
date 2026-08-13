---
name: abb-library-research
description: Resolve ABB external-library/API uncertainty for Effect, Svelte, Tauri, Specta, or tauri-specta when implementation, IPC contract, codegen, installed-version, or upstream-source behavior affects the work. Use lockfiles, installed or registry source, route cards, and Context7 for the active question only.
---

# ABB Library Research

Answer external-library questions that affect an ABB implementation or contract decision. Smallest source-backed answer — not a survey.

## Boundary

Resolved lockfile versions, installed or registry-packaged source, route cards, Context7, exact public package docs.

Does not own metadata, path safety, job lifecycle, IPC guardrails, or release.

Skip ABB-owned domains such as metadata, processing, output artifacts, audio
engine behavior, path safety, job lifecycle, IPC guardrails, or release unless
external-library truth is the active blocker. No durable artifacts. Do not
commit upstream source snapshots as research material. The patched FFmpeg sys
crate under `vendor/` is build provenance, not this skill.

## Answer

- **Behavior** for the active question
- **ABB route** (owning boundary)
- **Constraints** that change implementation
- **Version or path** when version-sensitive
- **Residual uncertainty** when evidence conflicts

Version-sensitive answers must cite ABB's resolved lockfile version or say the
version could not be proven. Record a commit SHA when exceptional upstream
source was used.

## Lookup order

1. ABB-owned truth: nearest `AGENTS.md`, ABB code and tests, generated
   contracts, `bun.lock`, `Cargo.lock`, and manifests. Lockfiles identify the
   resolved version and source. Manifests explain the intended range but do not
   override the lockfile.
2. Exact installed or registry-packaged source:
   - JavaScript: exported `node_modules` package files and TypeScript
     declarations; the exact npm package tarball when installed files are
     absent or incomplete.
   - Rust: the Cargo registry source selected by `Cargo.lock`. Verify crate
     version and checksum before treating it as installed truth.
3. Context7: one focused query; include the resolved ABB version. A version in
   the query is not proof that Context7 indexed that version. If the result is
   current-only, mismatched, or does not expose its indexed version, label that
   limitation and continue.
4. Exact public package documentation at the resolved version (`docs.rs`, npm,
   unpkg/jsDelivr). Effect `llms.txt` is current documentation, not versioned
   evidence.
5. Exceptional upstream retrieval: see `references/source-retrieval.md`. Use
   only when registry packages omit tests, examples, codegen internals, runtime
   implementation, or history required by the question.

Context7: known library ID + one query. No broad surveys unless asked.

| Need | ID |
| --- | --- |
| Svelte 5 | `/sveltejs/svelte` or `/websites/svelte_dev` |
| Tauri 2 | `/websites/v2_tauri_app` |
| Effect | `/llmstxt/effect_website_llms_txt` or `/effect-ts/effect` |
| Tauri shell | `/tauri-apps/tauri-plugin-shell` |

## References

`references/source-retrieval.md` for exceptional upstream clones.

Route cards: `effect.md`, `svelte.md`, `tauri.md`, `tauri-plugins.md`, `specta.md`, `tauri-specta.md`.

Pattern files: `references/pattern-<library>-<topic>.md` only after repeated need.

## Guardrails

- Installed or registry-packaged source over current upstream documentation
- `src/lib/tauri/*` is the Tauri IPC boundary
- Effect stays private to owning workflows until boundary changes
