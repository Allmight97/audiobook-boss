# Metadata Artifacts UI

## Public API Strip

- Import from `src/ui/metadataArtifacts`.
- Exports: `MetadataArtifactsIsland`, `refreshMetadataArtifacts`,
  `stageMetadataArtifactClear`, `METADATA_ARTIFACT_FIELDS`,
  `MetadataArtifactField`.

## Private Cluster

- Files: `state.svelte.ts`, `MetadataArtifactsIsland.svelte`, `__tests__/`.
- Owns inspect/clear display state for artifact fields (`album_sort`,
  `comment`, `track`, `disk`) on the single selected file.

## Hard Invariants

- Artifact fields are cleared ONLY through explicit per-field intent staged via
  `stageMetadataIntentPatch(filePath, { [field]: { op: 'clear' } })`; they must
  never enter `METADATA_DRAFT_FIELDS` (`src/ui/metadataSession/draft.ts`,
  private) — that exclusion is what makes normal form saves preserve them
  (pinned by the metadataSession contract test).
- Clears ride the normal pending-save mechanism (Cmd+S applies); this island
  never writes files directly.
- `refreshMetadataArtifacts()` is called from the FileList selection
  presentation flow (`src/ui/fileList/metadataPanel.ts`, alongside
  `updateTagPreview()`) and after staging a clear. Metadata caches in the
  Metadata Session are not reactive; do not read them from `$derived` without a
  refresh trigger.

## Done Criteria

- Field additions come with a backend clear-intent path
  (`MetadataIntentPatch` in `abb-metadata-core`), a UI clear test, and a
  real-file round-trip assertion in the media-execution lane.
