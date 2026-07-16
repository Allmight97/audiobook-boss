# UI Surface Directives

Each UI owner under `src/ui/<owner>/` keeps its own nested `AGENTS.md` where it
has real state, lifecycle, or contract truth (closest file wins). App Shell
composes chrome, the full-width file area, and popover overlay placement; it
does not own file import, selection, metadata, or WorkRuntime truth.

## Composition Surfaces

- `appShell` owns application chrome and top-level composition; its public
  strip and boundaries live in `src/ui/appShell/AGENTS.md`.
- `operationsBar` owns the bottom operations composition and its local display
  mode; its public strip and boundaries live in `src/ui/operationsBar/AGENTS.md`.
- `metadataSurface` owns the edit-surface presentation (persistent rail by
  default, anchored popover by preference) and composes the
  Metadata/Facts/Chapters/Output panes from their owning strips; it does not
  own metadata or output truth. Both presentations satisfy the FileList
  coordinator's `MetadataSurfacePresentation` contract; the rail presentation
  never steals focus.
- `FileImportIsland` remains the drag/drop and import-feedback wrapper around
  App Shell. There are no `leftColumn`, `encodingWorkbench`, or
  `metadataManager` composition shells.
