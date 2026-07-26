# FileList Directives

## Scope

- Applies to current file-list state, append/dedupe results, selection/order
  mutation, metadata draft staging for current selections, totals, and output
  refresh triggers under `src/ui/fileList/`.
- FileList owns pre-processing workbench state after backend import analysis has
  returned `FileListInfo`.

## Public API Strip

- Import file list runtime symbols from `src/ui/fileList`; do not reach into private files.
- Authoritative runtime export surface = `index.ts`, pinned by
  `__tests__/runtime-api-contract.test.ts`. Treat that test as the source of
  truth instead of a hand-listed export set here.
- Do **not** export `fileListSessionState`, selection internals, or event
  handlers from the index. Cross-module reads use `readX()` accessors from
  `viewState.svelte.ts` inside component `$derived(...)`.
- Cross-owner total display reads are `readFileListCount()`,
  `readCombinedDurationText()`, and `readCombinedSizeText()`; they return
  presentation-ready values without exposing session state.

## Private Cluster

- Files: `FileListIsland.svelte`, `actions.ts`, `state.svelte.ts`,
  `viewState.svelte.ts`, `events.ts`, `selection.ts`,
  `metadataStaging.ts`, `metadataPanel.ts`, `appendResult.ts`,
  `inspectorState.svelte.ts`, `keyboardNavigation.ts`,
  `coverThumbnails.svelte.ts`, `labFixtures.ts`, `__tests__/`.
- `labFixtures.ts` is a dev-only design-lab adapter (deterministic scenario
  seeding for `lab.html`); it is not exported from the runtime Public API
  Strip.

## Preferred Path

- Keep append/dedupe calculation in `appendResult.ts`. Import workflows may
  consume the returned outcome, but should not independently decide whether an
  analyzed import was duplicate-only.
- Keep metadata draft staging in `metadataStaging.ts`; selection/order actions
  call it before changing selection when dirty drafts must be preserved.
- Keep `actions.ts` focused on visible FileList mutations: display, append,
  select, reorder, remove, clear, lock, totals, and output refresh.
- `FileListIsland.svelte` owns list rendering; `FileImportIsland` composes it
  and owns import/drop/picker workflow.
- `coverThumbnails.svelte.ts` is FileList-private, ephemeral display cache.
  It may not read or mutate Metadata Session cover truth; schedule with the
  current list paths and treat `absent` as a stable placeholder state.
- Preserve `FileListInfo` truth from the backend. Do not add frontend-owned
  audio importability or supported-extension allowlists.
- Import-order (arrival) ordinal truth lives in `state.svelte.ts` and is
  assigned only by the display/append actions; restore behavior no-ops when
  ordinals are absent.

## Hard Invariants

- Adding files must preserve pending metadata drafts before mutating the visible
  list.
- Duplicate-only imports must not mutate the list and must surface one visible
  status path.
- Selection/order changes must preserve the selected file identity when moving
  or appending files.
- Cover-art preview/commit ownership is mode-keyed through `coverArt/coverOwner.ts`
  (merge → first valid input; batch → exactly one selected valid file; batch
  multi-select ignores cover-art commits); selection flows call
  `refreshCoverArtDisplay()` rather than gating on a global custom-cover flag.
- FileList mutations that can affect processing requests must refresh output
  estimates or previews through the output panel public surface.

## Done Criteria

- Append/dedupe changes have focused tests against `appendResult.ts` and at
  least one workflow-level import/FileList behavior test.
- Selection, order, metadata staging, and output-refresh behavior stay covered
  by targeted Vitest tests.
- Run targeted Vitest files when proving FileList workflow changes.
