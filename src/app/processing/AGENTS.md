# Processing

## Scope

- The engine builds, reviews, and runs exports and previews
  (`crates/abb-engine/src/session/AGENTS.md`). This owner starts them with the
  session's `submit` and `preview` intents, asks the Output collision dialog
  when the engine reports existing outputs, words the outcome, and runs the
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
  are the engine's. `submit.ts` sends intents and reads
  `output.submission`; it never builds a payload or decides a refusal.
- A `reviewRequired` status loops through the Output dialog, including a held
  review received when a replacement frontend attaches: the chosen policy
  goes back as `chooseCollisionPolicy`, a cancel as `cancelCollisionReview`.
- Each `output.restartOffers` entry is asked once in a native dialog, one at
  a time: Restart runs `restartTitle` through the same submission flow, Keep
  Location posts `keepTitleLocation`. Work Center also offers these actions
  on the matching operation/title row so a refused restart can be retried.
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

- `submit.test.ts` covers the review loop, refusal wording, cancellation, and
  terminal wording against a stub link. `preview.test.tsx` covers snapshot
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
