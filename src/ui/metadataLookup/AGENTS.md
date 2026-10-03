# Metadata Lookup Directives

## Scope

- Applies to the Solid lookup dialog under `src/ui/metadataLookup/`. The
  engine owns search, queue, and apply; `src/app/metadataLookup` shows them
  and schedules cover previews.

## Public API Strip

- Import `MetadataLookupView` from `src/ui/metadataLookup`.
- Applying a result and advancing the queue are lookup intents; the engine
  puts applied values in the form and stages them through the draft gate. Do
  not add a view-side apply or staging path.

## Hard Invariants

- Metadata lookup is a decision surface: visible result data needed to choose
  an action must load from app-owned state scheduling, not hover, focus, or
  scroll triggers.
- Cover-preview cache, listeners, and scheduler belong to the runtime Metadata
  Lookup owner. The view reads `coverPreview` and dispatches
  `scheduleCoverPreviews` / `cancelCoverPreviews`. Do not import private
  preview modules.
- Provider-controlled remote media URLs must not be rendered directly into DOM
  attributes. Cover previews route through the Tauri cover-art loader and
  render only app-owned data URLs from backend-validated bytes via
  `src/lib/media/coverArtDataUrl.ts`.

## Private Cluster

- Files: `MetadataLookupView.tsx`, `metadataLookup.css`,
  `__tests__/MetadataLookupView-modal.test.tsx`.

## Done Criteria

- Apply and queue rules are proved in the engine's session tests; the
  preview scheduler in its own test. Dialog focus/containment stays covered by
  the view modal test.
- Two-runtime preview isolation is owned by `src/app/runtime/runtime.test.ts`.
