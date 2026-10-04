# File Import Directives

## Scope

- Applies to the Solid file-import view under `src/ui/fileImport/`: picker
  buttons, native drop wiring, and Remote Source dialog mount. Files the OS
  opens are imported by the engine without this view. The engine analyzes imports and changes the list; this view reaches
  it through `src/app/inputSession`.

## Preferred Path

- Dispatch Input `importIntent` for pick-files, pick-folder, and path import.
  Do not add a parallel import workflow in this folder.
- Cover-art native drops dispatch Metadata `applyCoverArtDrop`.
- Compose `RemoteSourceAcquireView` next to the Import split button. Main click
  opens `settings.defaultAcquisitionLane`; caret picks Audible or Indexer via
  `remoteSource.open({ lane })`.

## Hard Invariants

- Import goes through Input's import intents so the engine validates paths
  and analyzes files.
- Import must not add files while processing order is locked. The engine
  retains and retries accepted OS-open requests after unlock; this view sends
  the request without a second retry flag.
- The engine imports acquired files; this view does not.

## Done Criteria

- UI-facing changes prove visible error/status behavior through Input Session
  public reads and intents.
- Run targeted Vitest files when proving import UI changes.
