# Metadata Lookup Directives

## Scope

- Applies to the Solid lookup dialog under `src/ui/metadataLookup/`. Search,
  queue, apply, and cover-preview scheduling live in
  `src/app/metadataLookup`.

## Public API Strip

- Import `MetadataLookupView` from `src/ui/metadataLookup`.
- Result application stages through the Metadata Session strip
  (`stageMetadataIntentPatch`). Do not add a lookup-private staging path.

## Hard Invariants

- Metadata lookup is a decision surface: visible result data needed to choose
  an action must load from app-owned state scheduling, not hover, focus, or
  scroll triggers.
- Cover-preview cache, listeners, and scheduler belong to the runtime Metadata
  Lookup owner. The view reads `coverPreview` and dispatches
  `scheduleCoverPreviews` / `cancelCoverPreviews`. Do not import private
  preview modules or restore `bumpPreview`.
- Provider-controlled remote media URLs must not be rendered directly into DOM
  attributes. Cover previews route through the Tauri cover-art loader and
  render only app-owned data URLs from backend-validated bytes via
  `src/lib/media/coverArtDataUrl.ts`.

## Private Cluster

- Files: `MetadataLookupView.tsx`, `metadataLookup.css`,
  `__tests__/MetadataLookupView-modal.test.tsx`.

## Done Criteria

- Preview scheduler and apply workflow stay covered by
  `src/app/metadataLookup` tests. Dialog focus/containment stays covered by
  the view modal test.
- Two-runtime preview isolation is owned by `src/app/runtime/runtime.test.ts`.
