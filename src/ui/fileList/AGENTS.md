# FileList Directives

## Scope

- Applies to the Solid file-list view, pointer reorder, and cover thumbnails
  under `src/ui/fileList/`. List truth, selection, and import live in
  `src/app/inputSession`.

## Public API Strip

- Import `FileListView` from `src/ui/fileList`.
- Do not reintroduce `fileListSessionState` or a second file list beside
  Input Session.

## Private Cluster

- Files: `FileListView.tsx`, `fileList.css`, `pointerReorder.ts`,
  `coverThumbnails.ts`.

## Preferred Path

- `FileListView` reads Input `view` and dispatches Input Session intents.
- Cover thumbnails are a presentation cache, not list truth.
- PDF companion chips subscribe to `subscribeRemoteSourceSupplementalAssets`.
  Do not assume an Input publish also rerenders supplemental assets.
- Remote session purge tracks Input file identity through Remote Source. Do
  not dual-purge from this view.

## Hard Invariants

- Do not add a parallel file-list store.
- Preserve listbox-scoped keyboard handling and pointer-reorder cleanup
  already owned by Input Session + `pointerReorder.ts`.

## Done Criteria

- Thumbnail and pointer-reorder changes have focused Vitest coverage.
- List mutation behavior is proved in `src/app/inputSession`.
