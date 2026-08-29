# FileList Directives

## Scope

- Applies to current file-list state, append/dedupe results, selection/order
  mutation, metadata draft staging for current selections, and totals under
  `src/ui/fileList/`.
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

## Private Cluster

- Files: `actions.ts`, `state.svelte.ts`, `viewState.svelte.ts`,
  `pointerReorder.ts`, `selection.ts`, `metadataStaging.ts`,
  `appendResult.ts`, `coverThumbnails.svelte.ts`, `__tests__/`.

## Preferred Path

- Keep append/dedupe calculation in `appendResult.ts`. Import workflows may
  consume the returned outcome, but should not independently decide whether an
  analyzed import was duplicate-only.
- Keep metadata draft staging in `metadataStaging.ts`; selection/order actions
  call it before changing selection when dirty drafts must be preserved.
- Keep `actions.ts` focused on visible FileList mutations: display, append,
  select, reorder, remove, clear, lock, and totals.
- `FileListIsland.svelte` owns list rendering; `FileImportIsland` composes it
  and owns import/drop/picker workflow.
- Preserve `FileListInfo` truth from the backend. Do not add frontend-owned
  audio importability or supported-extension allowlists.

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
- Duration and selection changes are visible through Input Session public
  atoms. Output Plan derives preview and estimate from those atoms; FileList
  does not poke Output.

## Done Criteria

- Append/dedupe changes have focused tests against `appendResult.ts` and at
  least one workflow-level import/FileList behavior test.
- Selection, order, and metadata staging stay covered by targeted Vitest tests.
- Run targeted Vitest files when proving FileList workflow changes.
