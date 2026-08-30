---
name: abb-library-research
description: Resolve ABB external-library/API uncertainty for Effect, Solid, Tauri, Specta, or tauri-specta when implementation, IPC contracts, codegen, resolved-version behavior, or upstream changes affect the work. Ground answers in lockfiles and exact package source; use focused documentation or primary-source discovery only as needed.
---

# ABB Library Research

Answer external-library questions that affect an ABB implementation or contract decision. Smallest source-backed answer — not a survey.

## Boundary

Resolved lockfile versions, installed or registry-packaged source, route cards,
exact public package docs, focused documentation, and primary-source discovery.

ABB owners retain metadata, processing, output artifacts, audio engine behavior,
path safety, job lifecycle, IPC guardrails, and release. Research an external
library only when its behavior is the active blocker, then return the answer to
the owning ABB boundary. No durable artifacts. Do not commit upstream source
snapshots as research material. The patched FFmpeg sys crate under `vendor/` is
build provenance, not this skill.

## Answer

- **Behavior** for the active question
- **ABB route** (owning boundary)
- **Constraints** that change implementation
- **Version or path** when version-sensitive
- **Residual uncertainty** when evidence conflicts

Version-sensitive answers must cite ABB's resolved lockfile version or say the
version could not be proven. Record a commit SHA when exceptional upstream
source was used.

Stop when exact-version evidence answers the active behavior, the ABB owner and
implementation constraints are named, and any research-aid evidence is either
reconciled to the resolved package or labeled current-only, mismatched, or
unproven.

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
3. Exact public package documentation at the resolved version (`docs.rs`, npm,
   unpkg/jsDelivr). Effect `llms.txt` is current documentation, not versioned
   evidence.
4. Supplementary research only when the preceding evidence does not answer the
   active question. Choose the narrowest lane:
   - Context7 for focused usage guidance about a known library and concept.
     Resolve the library ID live, then run one focused query that includes the
     resolved ABB version. Treat default-branch, mismatched, or undisclosed
     indexed versions as orientation rather than version proof.
   - Exa, when available, to discover primary documentation, source, issues,
     releases, or history when the route is unclear, Context7 is incomplete or
     version-mismatched, or the question crosses source types. Validate a
     selected primary source against ABB's resolved package or exact version.
   - Use both only when the first lane leaves decision-changing uncertainty.
     Search results and generated summaries are discovery evidence, not
     authority for installed behavior.
5. Exceptional upstream retrieval: see `references/source-retrieval.md`. Use
   only when registry packages omit tests, examples, codegen internals, runtime
   implementation, or history required by the question.

Context7 library IDs in route cards are selection hints, not a substitute for
live resolution. No broad surveys unless asked.

## References

`references/source-retrieval.md` for exceptional upstream clones.

Route cards: `effect.md`, `solid.md`, `tauri.md`, `tauri-plugins.md`, `specta.md`, `tauri-specta.md`.

Pattern files: `references/pattern-<library>-<topic>.md` only after repeated need.

## Guardrails

- Installed or registry-packaged source over current upstream documentation
- `src/lib/tauri/*` is the Tauri IPC boundary
- Effect workflow APIs stay private to owning workflows. Do not import
  `effect/unstable/reactivity` or `@effect/atom-solid`.
