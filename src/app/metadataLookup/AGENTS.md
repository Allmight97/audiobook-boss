# Metadata Lookup

## Scope

- The engine owns the lookup: queries, search, the queue of titles, applying a
  result, and superseding stale work (`crates/abb-engine/src/session/AGENTS.md`).
  This owner shows the lookup snapshot, words its typed status (`state.ts`),
  and sends lookup intents. The engine serves result covers.
- Unconfirmed query text is bound to its queued path and metadata binding;
  advancing or rebinding the lookup drops the previous title’s echo immediately.
- The Solid dialog lives in `src/ui/metadataLookup`. It renders this owner; it
  does not keep a second lookup store or cover cache.

## Public API Strip

- Import `createMetadataLookupOwner` and owner types from
  `src/app/metadataLookup`. `owner.ts` and `state.ts` are private.

## Testing

- `state.test.ts` pins status wording; `owner.test.ts` pins delayed query
  echoes across queue advancement.
- Lookup rules (queue, apply, supersession) are proved in the engine's
  session tests.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Adding a cover cache or loader here; covers load once, in the engine.
