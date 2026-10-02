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
- `index.ts` is the export surface. Do not import `runtime.ts`, `submit.ts`,
  `view.ts`, `domain/`, or `services/` from outside this owner.

## Hard Invariants

- Which titles go, their sources, audio, chapters, naming, and pending edits
  are the engine's. `submit.ts` sends intents and reads
  `output.submission`; it never builds a payload or decides a refusal.
- A `reviewRequired` status loops through the Output dialog: the chosen policy
  goes back as `chooseCollisionPolicy`, a cancel as `cancelCollisionReview`.
- Each `output.restartOffers` entry is asked once in a native dialog, one at
  a time: Restart runs `restartTitle` through the same submission flow, Keep
  Location posts `keepTitleLocation`.
- Previews run in the engine without WorkRuntime. `processing-progress` and
  `processing-queue` are preview events with no operation id; Work Operations
  consumes WorkRuntime snapshots for exports.
- Foreground cancel settles the local render only. Operation-scoped cancel
  lives in Work Operations.
- Consume the backend terminal verdict (`RunTerminalClass` on
  `ProcessCommandResult`) for preview completion. Do not re-derive terminal
  precedence from per-job rows.
- Preview duration lives in `PreviewAudioControls` screen-local Solid state.
  Submit goes through Processing `start`.
- Each Processing owner instance owns its status view store and
  `StatusPanelRuntime`. Disposing one runtime cannot publish into another.

## Testing

- `submit.test.ts` covers the review loop, refusal wording, cancellation, and
  opening a single finished preview against a stub link.
- `remote-source-boundary.test.ts` pins the visual Remote UI strip and proves
  production Processing does not import UI or private Remote implementation
  files.
- `runtime-api-contract.test.ts` pins this owner's public export strip.
- Status UI strip is pinned by `src/ui/statusPanel/__tests__/runtime-api-contract.test.ts`.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Building any part of an export request here instead of in the engine.
- Converting Status Panel into a WorkRuntime consumer.
- Adding a module-global status publisher.
