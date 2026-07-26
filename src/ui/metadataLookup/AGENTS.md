# Metadata Lookup Directives

## Scope

- Applies to metadata lookup modal search, result comparison, safe cover preview
  loading, result application, and queue advancement under `src/ui/metadataLookup/`.

## Public API Strip

- Import from `src/ui/metadataLookup`. No contract test pins this owner yet —
  keep this list in sync on purpose-changes.
- Exports: `applyMetadataLookupResult`, `closeMetadataLookup`,
  `initMetadataLookup`, `openMetadataLookup`, `searchMetadataLookup`,
  `skipMetadataLookupQueueItem`, `useManualMetadataEntryFromLookup`,
  `MetadataLookupIsland`.
- Result application stages through the Metadata Session strip
  (`stageMetadataIntentPatch`) — the same pending truth the primary save
  drains; do not add a lookup-private staging path.

## Hard Invariants

- Metadata lookup is a decision surface: visible result data needed to choose an
  action must load from app-owned state scheduling, not hover, focus, or scroll
  triggers.
- Provider-controlled remote media URLs must not be rendered directly into DOM
  attributes. Cover previews route through the Tauri cover-art loader and render
  only app-owned data URLs from backend-validated bytes via
  `src/lib/media/coverArtDataUrl.ts` (no local duplicate encoders).

## Private Cluster

- `actions.ts` — public orchestrator functions; preserves the dynamic
  `await import('./metadataLookupWorkflow')` cycle break (load-bearing — do
  not convert it to a static import).
- `metadataLookupWorkflow.ts` — workflow owner, co-located service contract and
  live layer (`satisfies`); see `src/lib/effect/AGENTS.md` for harness shape.
- `metadataLookupWorkflowDomain.ts` — apply/skip/queue domain helpers.
- `metadataLookupCoverPreview.svelte.ts` — safe preview scheduling and bytes
  loading.
- `state.svelte.ts` — lookup modal and queue `$state`.

## Done Criteria

- Preview scheduler, island rendering, and apply workflow behavior stay covered
  by focused Vitest tests.
