# FileList Directives

## Scope

- Applies to current file-list state, append/dedupe results, selection/order
  mutation, metadata draft staging for current selections, totals, and output
  refresh triggers under `src/ui/fileList/`.
- FileList owns pre-processing workbench state after backend import analysis has
  returned `FileListInfo`.

## Preferred Path

- Keep append/dedupe calculation in `appendResult.ts`. Import workflows may
  consume the returned outcome, but should not independently decide whether an
  analyzed import was duplicate-only.
- Keep metadata draft staging in `metadataStaging.ts`; selection/order actions
  call it before changing selection when dirty drafts must be preserved.
- Keep `actions.ts` focused on visible FileList mutations: display, append,
  select, reorder, remove, clear, lock, totals, and output refresh.
- Preserve `FileListInfo` truth from the backend. Do not add frontend-owned
  audio importability or supported-extension allowlists.

## Hard Invariants

- Adding files must preserve pending metadata drafts before mutating the visible
  list.
- Duplicate-only imports must not mutate the list and must surface one visible
  status path.
- Selection/order changes must preserve the selected file identity when moving
  or appending files.
- FileList mutations that can affect processing requests must refresh output
  estimates or previews through the output panel public surface.

## Done Criteria

- Append/dedupe changes have focused tests against `appendResult.ts` and at
  least one workflow-level import/FileList behavior test.
- Selection, order, metadata staging, and output-refresh behavior stay covered
  by targeted Vitest tests.
- Run targeted Vitest files before handoff for FileList workflow changes.
