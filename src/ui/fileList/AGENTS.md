# FileList Directives

## Scope

- Applies to the Solid file-list view, pointer reorder, and cover thumbnails
  under `src/ui/fileList/`. List truth, selection, and import live in
  `src/app/inputSession`.

## Public API Strip

- Import `FileListView` from `src/ui/fileList`.

## Private Cluster

- Files: `FileListView.tsx`, `AudioHandlingControl.tsx`, `fileList.css`,
  `pointerReorder.ts`, `coverThumbnails.ts`, `TitleSources.tsx`, `SelectedAudioSettings.tsx`.

`AudioHandlingControl` owns its transient hover/focus disclosure, pointer
travel grace, outside dismissal, and Escape cleanup. Its portal overlays the
list without changing row height. Hover/focus opens its audio-plan dialog; click
pins it. The dialog shows the plan the engine resolved for the title
(`Encoding.plan`): pending, resolved, failed with a reason, or needing a choice.
Settings, per-title and selected-title controls reuse EncoderView.
The title row shows the engine's size estimate, worded by Output Plan, beside
its audio summary; no estimate shows until the engine has one.
`SelectedAudioSettings` owns only its toolbar editor disclosure and closes when
selection changes or input locks. Bulk changes dispatch Encoding intents;
Apply App Settings copies the current defaults into the explicitly targeted titles. A copied plan uses a check and
explicit copy as well as blue. Source rows expose facts and order; audio
choices belong to the title. Locking input closes the dialog.
Title text selects and expands/collapses sources. `TitleSources` reuses the same
pointer reorder helper as the outer list, with a hit test scoped to that title.
List membership, source order, and audio choices are engine truth.

## Preferred Path

- `FileListView` reads Input `view` and dispatches Input Session intents.
  Row click/removal, keyboard Select all, Escape clear-highlight, and toolbar
  Clear go through awaitable `selectFile`, `removeFile`, `selectAll`,
  `clearSelection`, and `clearAllFiles` intents so the engine's draft gate
  runs.
- Cover thumbnails are a presentation resource, not list truth. Each
  `FileListView` owns a private thumbnail resource with its loader, reactive
  reads, bounded cache, and queue; dispose it with the view. Scheduling an
  empty list clears only that resource.
- PDF companion chips use Input's `hasCompanions`, which reads the engine's
  titles part.

## Hard Invariants

- Do not add a parallel file-list store.
- Do not apply selection or list membership in this view. Every
  selection-changing list action dispatches the Input intents above.
- Preserve listbox-scoped keyboard handling and pointer-reorder cleanup
  already owned by Input Session + `pointerReorder.ts`.

## Done Criteria

- Thumbnail and pointer-reorder changes have focused Vitest coverage.
- List rules are proved in the engine's session tests; what the list shows
  and sends is proved in `src/app/inputSession`.
- Lifetime migration proves two mounted File List views cannot cancel, clear,
  or publish thumbnail state into one another.
