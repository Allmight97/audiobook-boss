# Processing

## Scope

- The engine builds, reviews, and runs exports and previews
  (`crates/abb-engine/src/session/AGENTS.md`). This owner starts them with the
  session's `submit` and `preview` intents, renders the engine's decision facts,
  words outcomes arriving in snapshots, and runs the
  Status Panel for previews.
- Solid views live in `src/ui/statusPanel` and `src/ui/previewAudio`. They
  render this owner; they do not keep a second status or preview store.

## Public API Strip

- Import `createProcessingOwner` and owner types from `src/app/processing`.
- Workbench callers that only need the composed UI strip import
  `src/ui/statusPanel` instead.
- `index.ts` is the export surface. Do not import private `owner.ts`, `submit.ts`, `render.ts`, or
  `view.ts` from outside this owner.

## Hard Invariants

- Which titles go, their sources, audio, chapters, naming, and pending edits
  are the engine's. `owner.ts` sends intents; `submit.ts` words
  `output.submission`. Neither builds a payload or decides a refusal.
- Collision presentation reads the Output owner's held question. There is no
  frontend review loop or submission gate; busy facts and continuation are engine-owned.
- `restartPrompt` renders the engine's one eligible question, while `restartOffers`
  exposes retained offers for Work Center retry. Restart sends `restartTitle`,
  Keep Location sends `keepTitleLocation`, both with the shown title/revision.
  The engine owns which question comes next and whether an answered offer is
  automatically asked again; teardown sends neither answer. Snapshot-driven
  dialogs avoid a native prompt surviving replacement with a stale continuation.
- Preview identity, progress, queue rows, cancellation, and terminal truth come
  from `output.previewRun`. Render its operation snapshot; do not aggregate
  progress or listen for separate processing events.
- Cancel posts `cancelPreview` with the current run identity and optional child
  identity. The engine stops the corresponding work; disposal does not cancel it.
- Artwork is read by run identity after `artworkReady`. Ignore a cover reply for
  another run or a disposed view. A finished preview opens only the path the
  engine grants through `takePreviewOutput`; an accepted claim still opens if
  the requesting frontend is disposed before its reply arrives.
- Preview duration lives in `PreviewAudioControls` screen-local Solid state.
  Submit goes through Processing `start`.
- Each Processing owner instance owns its status view store and
  publisher. Disposing one runtime cannot publish into another.

## Testing

- `submit.test.ts` covers snapshot outcome presentation, busy facts and intent
  routing. Decision ordering/staleness is proved in engine session tests;
  `src/ui/collisionDialog/CollisionDialogView.test.tsx` covers answer wiring and
  teardown/reattachment without an implicit answer. `preview.test.tsx` covers snapshot
  reattachment, accepted artwork, run cancellation, and the output claim.
- `remote-source-boundary.test.ts` pins the visual Remote UI strip and proves
  production Processing does not import UI or private Remote implementation
  files.
- `runtime-api-contract.test.ts` pins this owner's public export strip.
- Status UI strip is pinned by `src/ui/statusPanel/__tests__/runtime-api-contract.test.ts`.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Building any part of an export request here instead of in the engine.
- Reintroducing frontend progress or terminal policy.
- Adding a module-global status publisher.
