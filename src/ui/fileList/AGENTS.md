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

- Files: `FileListView.tsx`, `AudioHandlingControl.tsx`, `fileList.css`,
  `pointerReorder.ts`, `coverThumbnails.ts`, `TitleSources.tsx`, `SelectedAudioSettings.tsx`.

`AudioHandlingControl` owns its transient hover/focus disclosure, pointer
travel grace, outside dismissal, and Escape cleanup. Its portal overlays the
list without changing row height. Hover/focus opens its audio-plan dialog; click
pins it. The dialog calls the shared Rust planner and ignores stale results.
Settings, per-title and selected-title controls reuse EncoderView.
The title row reads Output Plan's per-title size estimate beside its audio
summary. Recommended waits for the existing backend title preview; unknown
plans stay hidden until resolved; quality-based VBR shows “Size varies with audio”.
`SelectedAudioSettings` owns only its toolbar editor disclosure and closes when
selection changes or input locks. Bulk changes dispatch Encoding intents;
Apply App Settings copies the current defaults into the explicitly targeted titles. A copied plan uses a check and
explicit copy as well as blue. Source rows expose facts and order; audio
choices belong to the title. Locking input closes the dialog.
Title text selects and expands/collapses sources. `TitleSources` reuses the same
pointer reorder helper as the outer list, with a hit test scoped to that title.
List membership, source order, and persisted audio choices remain Input truth.

## Preferred Path

- `FileListView` reads Input `view` and dispatches Input Session intents.
  Row click/removal, keyboard Select all, Escape clear-highlight, and toolbar
  Clear go through awaitable `selectFile`, `removeFile`, `selectAll`,
  `clearSelection`, and `clearAllFiles` intents so the metadata draft gate runs.
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
