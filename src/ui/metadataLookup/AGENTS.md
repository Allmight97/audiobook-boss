# Metadata Lookup Directives

## Scope

- Applies to the Solid lookup dialog under `src/ui/metadataLookup/`. The
  engine owns search, queue, and apply; `src/app/metadataLookup` shows them.

## Public API Strip

- Import `MetadataLookupView` from `src/ui/metadataLookup`.
- Applying a result and advancing the queue are lookup intents; the engine
  puts applied values in the form and stages them through the draft gate. Do
  not add a view-side apply or staging path.

## Hard Invariants

- Metadata lookup is a decision surface: visible result data needed to choose
  an action must not wait on hover, focus, or scroll, so result covers load
  eagerly.
- Provider-controlled remote media URLs must not be rendered directly into DOM
  attributes. Result covers load from their `coverSrc` address, which the
  engine serves (`src/lib/tauri/coverSrc.ts`).

## Private Cluster

- Files: `MetadataLookupView.tsx`, `metadataLookup.css`,
  `__tests__/MetadataLookupView-modal.test.tsx`.

## Done Criteria

- Apply and queue rules are proved in the engine's session tests. Dialog
  focus/containment stays covered by the view modal test.
