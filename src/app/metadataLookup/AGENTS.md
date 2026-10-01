# Metadata Lookup

## Scope

- The engine owns the lookup: queries, search, the queue of titles, applying a
  result, and superseding stale work (`crates/abb-engine/src/session/AGENTS.md`).
  This owner shows the lookup snapshot, words its typed status (`state.ts`),
  sends lookup intents, and schedules the result thumbnails the dialog shows.
- The Solid dialog lives in `src/ui/metadataLookup`. It renders this owner; it
  does not keep a second lookup store or cover cache.

## Public API Strip

- Import `createMetadataLookupOwner` and owner types from
  `src/app/metadataLookup`. `owner.ts` and `state.ts` are private.

## Thumbnails

- Each owner instance owns its cover-preview scheduler and cache. Two live App
  Runtimes isolate preview state; disposing, cancelling, or clearing one
  cannot publish into the other.
- Views read `coverPreview` and dispatch `scheduleCoverPreviews` /
  `cancelCoverPreviews`. Previews are display only; the engine fetches the
  cover it applies.
- Provider-controlled remote media URLs are never rendered into DOM
  attributes. Previews route through the Tauri cover-art loader and render
  only app-owned data URLs.

## Testing

- `state.test.ts` pins status wording and query echo.
- `src/lib/media/__tests__/coverArtPreviewScheduler.test.ts` pins scheduler
  behavior, clear/Apply lifetime, and two-instance isolation.
- Two-runtime preview isolation lives in `src/app/runtime/runtime.test.ts`.
- Lookup rules (queue, apply, supersession) are proved in the engine's
  session tests.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Adding a module-global cover cache or listener set.
