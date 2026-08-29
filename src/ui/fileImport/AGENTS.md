# File Import Directives

## Scope

- Applies to the Solid file-import view under `src/ui/fileImport/`: picker
  buttons, native drop wiring, opened-file drain, and Remote Source dialog
  mount. Import analysis and list mutation live in `src/app/inputSession`.

## Preferred Path

- Dispatch `importIntentAtom` for pick-files, pick-folder, path import, and
  opened-file drain. Do not add a parallel import workflow in this folder.
- Cover-art native drops dispatch `applyCoverArtDropAtom`.
- Compose `RemoteSourceAcquireView` next to the Import from Library button.
  Subscribe to `remoteSourceLifetimeAtom` so remote session purge tracks Input
  file identity.

## Hard Invariants

- Import must not bypass backend audio path validation or `FileListInfo`
  analysis.
- Import must not add files while processing order is locked.
- Do not import a leftover file-list store for Remote handoff.

## Done Criteria

- UI-facing changes prove visible error/status behavior through Input Session
  public atoms.
- Run targeted Vitest files when proving import UI changes.
