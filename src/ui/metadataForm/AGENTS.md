# Metadata Form

## Scope

- Applies to metadata text-field state, multi-select field actions, dirty-field
  reads, metadata-form preview values, validation warning display state, and
  `MetadataFormFieldsIsland` rendering under `src/ui/metadataForm/`.

## Public API Strip

- Import metadata form runtime symbols from `src/ui/metadataForm`; do not reach
  into private files.
- Authoritative runtime export surface = `index.ts`, pinned by
  `__tests__/runtime-api-contract.test.ts`. Treat that test as the source of
  truth instead of a hand-listed export set here.
- Composition shells read `{ mode, selectionCount }` through
  `readMetadataFormViewSnapshot()`.
- Runtime callers apply series/subseries validation warning state through
  `applyMetadataFormValidationWarnings(metadata, errors)`.
- Tag Preview reads form-derived tag inputs through
  `readMetadataFormPreviewValues()`.
- Async metadata transitions guard prepare -> commit work with
  `readMetadataFormRevision()` so stale validation cannot reset newer edits.

## Private Cluster

- Files: `MetadataFormFieldsIsland.svelte`, `index.ts`,
  `previewState.svelte.ts`, `state.svelte.ts`, `__tests__/`.
- The cluster owns form field state, multi-select action state, warning display
  state, form-to-draft reads, form population, dirty-state tracking, and preview
  value publication for Tag Preview.

## Hard Invariants

- `metadataFormState` stays private to this owner and owner tests. Other owners
  use the Public API Strip.
- Metadata Form does not own metadata intent staging, backend validation, save
  workflow truth, lookup queue truth, cover-art bytes, or output naming
  preflight.
- Validation warnings shown beside series/subseries fields are represented here;
  the caller that owns validation timing supplies backend validation errors.
- Multi-select `blank` keeps a deliberate clear distinct from a missing value.

## Testing State

- Runtime export changes update
  `src/ui/metadataForm/__tests__/runtime-api-contract.test.ts`.
- Behavior changes use focused Vitest coverage for form reads, dirty state,
  multi-select action behavior, or island rendering as appropriate.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Letting another owner import `state.svelte.ts` or `previewState.svelte.ts`
  except through the documented strip.
- Moving validation warning display rules out of Metadata Form without a new
  owner decision.
