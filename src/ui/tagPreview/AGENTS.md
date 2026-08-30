# Tag Preview

## Scope

- Applies to the Solid tag-preview grid under `src/ui/tagPreview/`.
- Tag values are derived by Metadata Session. This owner renders them.

## Public API Strip

- Import from `src/ui/tagPreview`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.
- Exports: `TagPreviewView`.

## Private Cluster

- Files: `TagPreviewView.tsx`, `rows.ts`, `tagPreview.css`.

## Cross-Strip Coupling

- `TagPreviewView` reads Metadata Session `view().tags`.
- TSOA calculation and tag-field projection live in
  `src/app/metadataSession/tags.ts`. Do not add a local tag store, refresh
  function, or listener that copies those values.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing a push or snapshot API that writes tag values outside
  Metadata Session.
