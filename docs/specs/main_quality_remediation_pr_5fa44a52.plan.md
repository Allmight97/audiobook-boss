---
name: Main Quality Remediation PR
overview: Implement the main-branch quality remediation as a single PR with one commit per workblock, ordered and reshaped per blast-radius scout findings, with justifications grounded in the new AGENTS philosophy (one invariant, one owner, split when understanding or testing gets expensive — not LOC counts).
todos:
  - id: wb6-http-dedup
    content: "Commit 1 (WB6): Shared Audible HTTP module, parameterized policies, unify ensure_not_cancelled"
    status: completed
  - id: wb7-rust-godfiles
    content: "Commit 2 (WB7): Split acquisition.rs/audible mod harness; dedupe metadata-core patch application"
    status: completed
  - id: wb1-filelist-derive
    content: "Commit 3 (WB1): Delete fileList dom.ts; accessor-derived view state; immutable files updates; island-local drag"
    status: completed
  - id: wb2-dead-surface
    content: "Commit 4 (WB2): Shrink fileList/outputPanel strips; remove ChapterSpec hack; sync contracts/AGENTS"
    status: completed
  - id: wb3-helper-dedup
    content: "Commit 5 (WB3): Single chunked dataURL helper; pathBasename with fallback param + pathSegments"
    status: completed
  - id: wb4-effect-ceremony
    content: "Commit 6 (WB4): Collapse Services/Live triples with satisfies+Pick aliases; ProcessingWorkflow keeps deps.ts cycle break"
    status: completed
  - id: wb5-boundary-leaks
    content: "Commit 7 (WB5): sessionAssets-only remoteSource strip; jobControls/appSettings routing; atomic root-file moves"
    status: completed
  - id: wb8-projection-audio
    content: "Commit 8 (WB8): statusPanel projection thinning; audio orchestrator extraction; drop clippy allows"
    status: completed
isProject: false
---

# Main Quality Remediation — Single PR, One Commit Per Workblock

Branch `codex/main-quality-remediation` off `main` (`d93b0b67`). One PR; each workblock lands as a self-contained commit that builds and passes tests on its own. After each commit, a scout pass re-checks the ≤3-hop blast radius before moving on.

## Scout findings that changed the plan

Three read-only scouts mapped blast radius for every workblock ([WB1 scout](c2e89983-dba3-4621-a37b-fd3902fa1485), [WB2/3/5 scout](3eaeb703-1f42-426b-8df5-d9ca669f48c5), [WB4/6/7 scout](49e063c0-e718-45e8-9046-9e46ffb726c0)). Material changes:

- **Commit order is constraint-driven**: WB6 must precede WB7 (both edit `acquisition.rs`); WB1 before WB2 (both edit `fileList/index.ts`/`actions.ts`); WB4 before WB8 (both touch statusPanel).
- **HIGH: ProcessingWorkflow co-location risks an import cycle** — `processingWorkflowLive` → `fileList` barrel → `actions.ts` → `statusPanel` → `controller` → `processing` → back to `processingWorkflow.ts`. Today the cycle is broken only by the dynamic `import('./processingWorkflowLive')` at `processingWorkflow.ts:474`. WB4 keeps a per-owner `*.deps.ts` (dynamic-imported) for ProcessingWorkflow instead of full co-location.
- **HIGH: `typeof liveServices` widens `Pick<>` narrowings** (6 Services files narrow `console`/`feedback`), which would break test fakes like `{ showError: vi.fn() }`. WB4 uses `satisfies` + explicit narrow type aliases kept for harnesses.
- **HIGH: naive shared `pathBasename` breaks `statusPanel/services/fileLookup.ts:4`** (empty-string fallback used for filename equality, not display). Helper gets a `fallback: 'path' | 'empty'` parameter; `formatting.ts:98` is multi-segment and gets `pathSegments()` instead.
- **HIGH: a broad `remoteSource/index.ts` barrel creates a cycle** (`acquisitionWorkflow` → `fileImport/handlers` → `fileList` → `actions` → `remoteSource`). The strip exports **sessionAssets symbols only** — never `acquisitionWorkflow` or islands.
- **WB1 reactivity**: no module-level `$derived` exists anywhere in `src/` — the proven repo pattern is `readX()` accessors consumed via `$derived(readX())` in components (outputPanel precedent). Also, in-place `files.splice` mutations relied on `dom.ts`'s shallow copy; actions switch to immutable `files` replacement to guarantee reactivity.

## AGENTS reframe (post-d93b0b67)

No workblock is justified by LOC. Justifications now:

- **WB7 splits** (`acquisition.rs`, `supplemental_pdf.rs`, `audible/mod.rs`): inline test harnesses and mixed concerns (license, download, materialization, PDF) make each invariant expensive to locate and test in isolation; ownership is blurred across the orchestration. `metadata_lookup/service.rs`-style large _owned orchestrators_ stay intact.
- **WB4 collapse**: the Services/Live/Workflow triple is ceremony without an ownership boundary — one invariant (the workflow's effect contract) split across three files raises understanding cost.
- **Anything that is merely "big but owned" is dropped from scope.**

## Commit sequence

### Commit 1 — WB6: Shared Audible HTTP download module

New module under `src-tauri/src/remote_source/providers/audible/http/` (visibility `pub(super)`): client factory, redirect handling (parameterized: built-in follow + HTTPS guard for audio vs `Policy::none()` + manual loop for PDF), streaming to `StagedTempFile`, cancellation checks, redacted diagnostics. Parameterize cookies/auth, size cap (`MAX_SUPPLEMENTAL_PDF_BYTES`), progress callbacks, and the PDF `allow_insecure_for_test` hook — do not force one policy. Unify the duplicate `ensure_not_cancelled` (`acquisition.rs:170`, `materializer/mod.rs:553`) into one owner; re-export `pub(super)` for `license.rs`/`audio_download.rs`. `StagedTempFile` stays in `scoped_output.rs`.
Verify: `cargo nextest run -p audiobook-boss --lib`, clippy.

### Commit 2 — WB7: Backend decomposition on cohesion grounds

Split `providers/audible/acquisition.rs` into submodules (`acquisition/{mod,title,progress}` shape) keeping all `pub(super)` paths stable for `license.rs`, `materialization.rs`, `supplemental_pdf.rs`; relocate inline tests next to the code they prove. Move probe/test harness out of `audible/mod.rs`. In `crates/abb-metadata-core`, extract the shared patch-field application duplicated between `apply_to_metadata` and `to_write_plan` (`lib.rs:252-291` vs `298-351`) — crate-internal only, sole method caller is `metadata/intent_plan.rs`.
Verify: `cargo nextest run -p audiobook-boss --lib`, `cargo nextest run -p abb-metadata-core`, clippy.

### Commit 3 — WB1: fileList derived view state, delete dom.ts

Delete `src/ui/fileList/dom.ts`. Replace `viewState.svelte.ts` writable mirror with `readX()` accessors deriving from session state (repo-proven pattern; module `$derived` is unproven here). Switch `actions.ts` to immutable `files` replacement (scout H1: in-place splice reactivity unproven). Drag state (`draggedIndex`/`hoveredIndex`) becomes island-local `$state` in `FileListIsland.svelte` with thin `events.ts` drag handlers receiving setters. `hasFiles` derived from session; drop `setFileImportHasFiles`. `readCombinedSizeText()` re-derives from `totalSize`, preserving the `'--- MB'` null sentinel pinned by the contract test. Rewrite `order-lock-lifecycle.test.ts` view assertions to drive `setFileOrderLocked`/session instead of dom push helpers; remove dead `vi.mock('../dom')` from 3 tests. Add one counterexample test: session reorder/remove → derived view updates. Sync `fileList/AGENTS.md`.
Verify: `npx vitest run src/ui/fileList src/ui/__tests__`, `tsc --noEmit`.

### Commit 4 — WB2: Dead surface removal

Shrink `fileList/index.ts` to the 13 production-consumed exports (scout-confirmed kill-list of ~29 values + 2 types is production-dead); rewire ~15 test files to import cluster internals directly; update `runtime-api-contract.test.ts` expected list. Remove `readOutputDisplaySnapshot`/`getCurrentMetadata`/`OutputDisplaySnapshot` from outputPanel strip + contract test; fix stale `outputPanel/AGENTS.md` guidance (mandates an accessor no component uses). Drop `#[allow(unused_imports)] pub use passthrough::ChapterSpec` (scout: zero consumers outside `metadata/passthrough.rs` tests); sync `metadata/AGENTS.md`. Light `docs/system-map.md` touch.
Verify: vitest full run, `cargo check`, contract tests.

### Commit 5 — WB3: Helper dedup with semantics preserved

One chunked `coverArtBytesToDataUrl` (canonical = `coverArt.ts` impl: chunked 0x8000, WebP RIFF+WEBP check) in a shared location; delete the duplicates in `metadataLookupCoverPreview.svelte.ts` and `statusPanel/formatting.ts`; add a counterexample test covering JPEG/PNG/WebP sniff for the `coverArtTracker` path (its mime sniffing changes slightly — intended). `pathBasename(path, { fallback })` honoring `fileLookup.ts`'s empty-string semantics and `CollisionDialog`'s trailing-slash filtering; `pathSegments()` for `formatting.ts:98`; replace ~9 sites.
Verify: vitest full run.

### Commit 6 — WB4: Effect workflow ceremony collapse

For the 6 owners: delete `*WorkflowServices.ts`/`*WorkflowLive.ts`; co-locate live object + layer with the workflow, deriving service types via `satisfies` + explicit narrow aliases (preserve the 6 `Pick<>` narrowings so test fakes keep typechecking). First move orphaned domain types (`OutputPlanReviewRequest/Result`, `ImportAnalysisWorkflowAction`, etc.) into the `*Workflow.ts` strips. Order within commit: OutputPlan → ProcessingCancellation → MetadataLookup → MetadataSave (fold `metadataSaveWorkflowEntryLive.ts`) → ImportAnalysis → ProcessingWorkflow **last**, keeping a dynamic-imported `processingWorkflow.deps.ts` to preserve the cycle break at `processingWorkflow.ts:474`. Update `lib/effect/AGENTS.md` catalog + owner AGENTS files.
Verify: vitest full run, `tsc --noEmit`.

### Commit 7 — WB5: Boundary leaks and strips

`remoteSource/index.ts` exporting **sessionAssets symbols + types only** (no workflow/island barrel — cycle hazard); migrate the 7 private importers. `coverArt.ts` → use `getJobType()` from jobControls strip (exists already; also migrate `fileList/metadataPanel.ts`'s direct `jobControlsState.jobType` reads — same smell, scout-flagged). Route `encoderPanel/logic.ts` + `outputPanel/actions.ts` through `appSettings/index.ts` (re-exports already exist; 2-line changes). Atomic moves `coverArt.ts` → `coverArt/index.ts` and `metadataForm.ts` → `metadataForm/index.ts` (bundler resolution confirmed transparent; no `.js`-suffixed imports; `vi.mock` specifiers stay valid — but never leave file + dir coexisting). Add `remoteSource/AGENTS.md`.
Verify: vitest full run, `tsc --noEmit`, grep for residual private imports.

### Commit 8 — WB8: statusPanel projection + audio orchestrator

Thin the statusPanel multi-hop view projection (after WB4 so workflow files are stable); merge `feedback.ts` policy into the projection owner where cohesion improves. Audio orchestrator extraction in `src-tauri/src/audio/` and drop now-unneeded clippy allows — justified by ownership clarity, not size. Scope here stays conservative: only changes whose invariant/owner is nameable; anything else is dropped rather than churned.
Verify: vitest statusPanel suites, `cargo nextest run -p audiobook-boss --lib`, clippy.

## Scout protocol per commit

After each commit: re-run the relevant scout queries (importer grep ≤3 hops from changed files, contract tests, AGENTS sync check) before starting the next workblock. Any new hazard found gets classified fix/defer/reject in the commit message.

## Final verification

Full suite: `npx vitest run`, `tsc --noEmit`, `cargo nextest run` (both crates), clippy, `npm run bindings:check:runtime-boundary`, `git diff --check`. PR description lists per-commit scope, hazards encountered, and residual risk.
