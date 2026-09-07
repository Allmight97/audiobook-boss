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
  Row click, keyboard Select all, Escape clear-highlight, and toolbar Clear
  go through the awaitable `selectFile`, `selectAll`, `clearSelection`, and
  `clearAllFiles` intents so the metadata draft gate can run.
- Cover thumbnails are a presentation resource, not list truth. Each
  `FileListView` owns a private thumbnail resource with its loader, reactive
  reads, bounded cache, and queue; dispose it with the view. Scheduling an
  empty list clears only that resource.
- PDF companion chips use the runtime Remote Source owner's reactive
  `hasCompanions` read; supplemental changes can arrive after Input publishes.
- Remote session purge tracks Input file identity through Remote Source. Do
  not dual-purge from this view.

## Hard Invariants

- Do not add a parallel file-list store.
- Do not apply selection or list membership in this view. Every
  selection-changing list action dispatches the Input intents above.
- Preserve listbox-scoped keyboard handling and pointer-reorder cleanup
  already owned by Input Session + `pointerReorder.ts`.

## Done Criteria

- Thumbnail and pointer-reorder changes have focused Vitest coverage.
- List mutation behavior is proved in `src/app/inputSession`.
- Lifetime migration proves two mounted File List views cannot cancel, clear,
  or publish thumbnail state into one another.
