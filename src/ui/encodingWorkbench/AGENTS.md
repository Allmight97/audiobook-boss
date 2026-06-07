# Encoding Workbench

## Public API Strip
- Import the workbench renderer from `src/ui/encodingWorkbench`.
- Exports: `EncodingWorkbenchIsland`.

## Private Cluster
- Files: `EncodingWorkbenchIsland.svelte`.
- The cluster owns only the right-column encoder/output/tags composition.
  Encoder, output, tag-preview, and preview-audio state/action truth stays in
  the owning UI modules.

## Allowed Agent Edits Without Escalation
- Change workbench layout and visual composition when focused UI tests and a
  browser visual pass stay green.
- Compose existing public UI islands; do not import private `state.svelte.ts`
  or logic modules from encoder, output, or tag preview owners.

## Breaking-Change Triggers
- Moving processing request truth into this cluster.
- Letting this cluster alter Status Panel, Work Center, file management, or
  processing runtime behavior.
