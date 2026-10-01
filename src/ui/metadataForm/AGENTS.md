# Metadata Form

## Scope

- Applies to the Solid metadata text-field view under `src/ui/metadataForm/`.
  The engine owns form values, dirty state, validation, and Save; the view
  reads them through `src/app/metadataSession`.

## Public API Strip

- Import `MetadataFormView` from `src/ui/metadataForm`.

## Private Cluster

- Files: `MetadataFormView.tsx`, `metadataForm.css`.

## Hard Invariants

- Do not add a parallel form store beside Metadata Session.
- Metadata Form does not own intent staging, backend validation, lookup queue
  truth, or cover-art bytes.

## Done Criteria

- Field rules are proved in the engine's session tests; what the form shows
  and sends is proved by Metadata Session owner tests and DOM tests of this
  view.
