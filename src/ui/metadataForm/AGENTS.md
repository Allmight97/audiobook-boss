# Metadata Form

## Scope

- Applies to the Solid metadata text-field view under `src/ui/metadataForm/`.
  Form truth, dirty state, and save live in `src/app/metadataSession`.

## Public API Strip

- Import `MetadataFormView` from `src/ui/metadataForm`.

## Private Cluster

- Files: `MetadataFormView.tsx`, `metadataForm.css`.

## Hard Invariants

- Do not add a parallel form `$state` store beside `metadataViewAtom`.
- Metadata Form does not own intent staging, backend validation, lookup queue
  truth, or cover-art bytes.

## Done Criteria

- Field edit, multi-select action, and save behavior are proved through
  Metadata Session owner tests plus focused DOM tests of this view.
