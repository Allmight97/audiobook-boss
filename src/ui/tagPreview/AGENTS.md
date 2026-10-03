# Tag Preview

## Scope

- Applies to the Solid tag-preview grid under `src/ui/tagPreview/`.
- The engine derives the tag values (`MetadataSnapshot.tags`); Metadata
  Session maps them to row names. This owner renders them.

## Public API Strip

- Import from `src/ui/tagPreview`. The runtime export surface is `index.ts`,
  pinned by `__tests__/runtime-api-contract.test.ts`.

## Private Cluster

- Files: `TagPreviewView.tsx`, `rows.ts`, `tagPreview.css`.

## Cross-Strip Coupling

- `TagPreviewView` reads Metadata Session `view().tags`.
- `src/app/metadataSession/tags.ts` maps the engine's tag preview to row
  names; title-to-album, author-to-album-artist, and TSOA are the engine's.
  With several titles selected the TSOA row omits each
  title's source value, so it can show blank where outputs keep their own. Do
  not add a local tag store, refresh function, or listener
  that copies those values.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing a push or snapshot API that writes tag values outside
  Metadata Session.
