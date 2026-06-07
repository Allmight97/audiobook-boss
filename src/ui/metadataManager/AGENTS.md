# Metadata Manager

## Public API Strip
- Import the metadata manager renderer from `src/ui/metadataManager`.
- Exports: `MetadataManagerIsland`.

## Private Cluster
- Files: `MetadataManagerIsland.svelte`.
- The cluster owns only the right-column metadata composition. Cover-art,
  metadata-form, metadata lookup, and metadata-save truth stays in the owning
  UI modules.

## Allowed Agent Edits Without Escalation
- Change metadata-manager layout and visual composition when focused UI tests
  and a browser visual pass stay green.
- Compose existing public UI islands and action handlers; do not duplicate
  metadata, cover-art, or save workflow state.

## Breaking-Change Triggers
- Moving metadata save, lookup, cover-art loading, or field-state truth into
  this cluster.
- Letting this cluster alter processing, Status Panel, Work Center, file
  management, or runtime behavior.
