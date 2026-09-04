# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, acquisition,
  progress, cancellation, supplemental-asset retention/purge, and the Input
  handoff under `src/app/remoteSource/`.
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- Import session-asset coordination from `src/ui/remoteSource`.
- Import acquire intents, lane helpers (`providerIdFromLane`, `laneFromProviderId`),
  release/title filter helpers, and the Input handoff result type from
  `src/app/remoteSource`.
- `RemoteSourceOwner`: `open({ lane? })`, `selectLane`, `view`, workflow
  `runAction`, and indexer connection settings intents (`loadIndexerConnectionSettings`,
  `saveIndexerConnectionSettings`, `testIndexerConnection`, `indexerConnection`).
- `index.ts` is the export surface. Do not import `workflow.ts`,
  `sessionAssets.ts`, or `state.ts` from outside this owner.
- State/listeners, workflow generation counters, cover-preview scheduling, and
  Supplemental Asset maps are still module-global compatibility state. Do not
  add callers, a reset API, another global, or a UI re-export; new state belongs
  to each Remote Source owner.

## Hard Invariants

- Closing the dialog does not cancel an in-flight acquisition. Polling and
  selected hidden titles survive ordinary close. Lane switches reset Indexer
  results and Audible selection UI only; they do not cancel in-flight Audible
  acquisition. App disposal and native cancel/purge remain the cleanup authorities.
- Acquisition poll patches publish through the Remote Source owner view.
  Native jobs provide a progress snapshot from job creation;
  `RemoteSourceAcquireView` renders its live percentage and Cancel.
  File List and the inspector observe Supplemental Assets through the composed
  Remote Source owner so PDF chips update after Input has already published the
  new files. The current `subscribeRemoteSourceSupplementalAssets` export is a
  compatibility rail, not the target interface.
  Cancellation and app disposal invalidate the active acquisition generation
  so late Promise completions cannot overwrite terminal or reset state.
- Materialized audio becomes a normal Input session through
  Input `importIntent`. Do not call `handleImportedAudioPaths` or
  `getCurrentFileList`. If Input import is blocked or fails, purge the staged
  remote session immediately.
- Supplemental assets are keyed by the imported file `inputId`, not by
  provider path after handoff. Do not add a second file-list store.
- Remote Source purges sessions for input ids that leave the public Input
  view. File Import keeps that lifetime subscription alive.
- Frontend state may hold provider-neutral account, title, job, and
  diagnostic text. It must not persist credentials, tokens, cookies, license
  material, or raw provider payloads.

## Testing

- `sessionAssets.test.ts` pins input-id rekey, companion summaries that omit
  paths, retainer deferral, and shared-job purge.
- `workflow.test.ts` pins successful Input handoff, blocked-import purge,
  close-does-not-cancel, lane switch without cancelling Audible jobs, Indexer
  grab success/failure, unconfigured Indexer hydrate, `open({ lane })` reset
  patch semantics, and publication of polled `getAcquisitionStatus` snapshots.
- `display.test.ts` pins terminal classification so polling cannot spin forever.
- `selection.test.ts` pins filter/selection policy.
- `RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent and
  the polled owner-to-Solid progress path.
- When lifetime ownership changes, add two-runtime proof covering the affected
  state or resource: disposing A cannot cancel, purge, reset, or publish into B.

## Breaking-Change Triggers

- Adding a caller to the public session-asset compatibility exports.
- Dual-writing `fileListSessionState` or adding a parallel remote file list.
- Cancelling acquisition from dialog close.
