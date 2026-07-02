# Tag Preview

## Scope

- Applies to TSOA (Album Sort) calculation and the tag preview grid under
  `src/ui/tagPreview/`.

## Public API Strip

- Import from `src/ui/tagPreview`. No contract test pins this owner yet —
  keep this list in sync on purpose-changes.
- Exports: `calculateTSOA`, `updateTagPreview`, `initTagPreview`,
  `TagPreviewIsland`.

## Private Cluster

- Files: `state.svelte.ts` (tag preview values `$state`), `rows.ts`,
  `TagPreviewIsland.svelte`.

## Cross-Strip Coupling

- Tag preview values render from the metadata form's preview values, read
  through `readMetadataFormPreviewValues()` on the metadataForm Public API
  Strip (pull model: `updateTagPreview()` snapshots on each call). Do not
  import metadataForm private state (`previewState.svelte.ts`) directly.
- metadataForm pushes refreshes by calling `updateTagPreview()`; this pair is
  an intentional index↔index cycle that is safe because both sides only call
  at runtime — do not add module-init-time reads across it.
