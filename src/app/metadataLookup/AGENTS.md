# Metadata Lookup

## Scope

- Owns online metadata search, queue apply, and cover-preview scheduling
  under `src/app/metadataLookup/`.
- Solid dialog lives in `src/ui/metadataLookup`. It renders this owner; it
  does not keep a second lookup store or cover cache.

## Public API Strip

- Import `createMetadataLookupOwner` and owner types from
  `src/app/metadataLookup`.
- `index.ts` is the export surface. `workflow.ts`, `coverPreview.ts`,
  `state.ts`, and `services.ts` are private implementation modules.
- Keep the Effect fake layer private: workflow tests import
  `makeMetadataLookupWorkflowServicesLayer` from `workflow.ts`. Do not
  re-export cover-preview globals, `bumpPreview`, or a module-global cache.

## Hard Invariants

- Each Metadata Lookup owner instance owns its cover-preview scheduler and
  cache. Two live App Runtimes isolate preview state. Disposing, cancelling,
  or clearing A cannot publish into B.
- Views read `coverPreview` and dispatch `scheduleCoverPreviews` /
  `cancelCoverPreviews`. Do not restore `bumpPreview`, `previewRevision`,
  or a subscribe-to-global handshake.
- Apply joins the owner's in-flight preview request through
  `loadLookupCoverBytes`. Do not fetch cover bytes around the scheduler.
- Provider-controlled remote media URLs must not be rendered directly into
  DOM attributes. Previews route through the Tauri cover-art loader and
  render only app-owned data URLs.

## Testing

- `coverPreview.test.ts` pins scheduler behavior and two-instance isolation.
- Workflow tests inject `makeMetadataLookupWorkflowServicesLayer` with a
  harness-owned preview factory.
- Two-runtime preview isolation lives in `src/app/runtime/runtime.test.ts`.
- Status UI strip is pinned by `src/ui/metadataLookup` modal/island tests.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Restoring a module-global cover cache, listener set, or `bumpPreview`.
