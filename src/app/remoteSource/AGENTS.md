# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, acquisition,
  progress, cancellation, supplemental-asset retention/purge, and the Input
  handoff under `src/app/remoteSource/`.
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- `index.ts` is the export surface. Callers consume the composed
  `RemoteSourceOwner`; private state, workflow, assets, and previews stay here.
- `open({ lane? })` hydrates account/library state without a mounted view.
  `selectLane`, title/PDF/release selection intents, and workflow actions own
  transitions. `editSearch` accepts only user-editable search/filter fields;
  connection edits accept only URL, API-key, and category drafts.
- State, workflow generations, cover previews, and supplemental assets belong
  to each owner instance. Reset/disposal invalidates that instance's work.
- Nonvisual callers use the owner's companion, processing-asset, retention,
  reconciliation, and terminal-work intents; the UI strip exports only its view.

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
  new files. Consumers use the owner's reactive companion reads.
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
- Release selection is the `(indexerId, guid)` pair; GUID alone is not unique
  across indexers. Grab queues externally and never calls the Input handoff.
- Frontend state may hold provider-neutral account, title, job, and
  diagnostic text. It must not persist credentials, tokens, cookies, license
  material, or raw provider payloads.

## Testing

- `sessionAssets.test.ts` pins input-id rekey, companion summaries that omit
  paths, retainer deferral, and shared-job purge.
- `workflow.test.ts` pins successful Input handoff, blocked-import purge,
  close-does-not-cancel, lane switch without cancelling Audible jobs, Indexer
  grab success/failure, unconfigured Indexer hydrate, same-lane reopen preservation and cross-lane reset
  semantics, and publication of polled `getAcquisitionStatus` snapshots.
- `indexerConnection.test.ts` pins draft-only Test, write-only key behavior, successful
  Save refreshing open Indexer account state, and delayed loading preserving edits.
- `display.test.ts` pins terminal classification so polling cannot spin forever.
- `selection.test.ts` pins filter/selection policy and Indexer release seeder
  order.
- `src/ui/remoteSource/RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent,
  the polled owner-to-Solid progress path, Indexer release protocol/category
  tags, and Enter-to-search on the author and title fields.
- When lifetime ownership changes, add two-runtime proof covering the affected
  state or resource: disposing A cannot cancel, purge, reset, or publish into B.

## Boundary Changes

- Exposing internal state mutation or moving asset coordination into the UI.
- Dual-writing `fileListSessionState` or adding a parallel remote file list.
- Cancelling acquisition from dialog close.
