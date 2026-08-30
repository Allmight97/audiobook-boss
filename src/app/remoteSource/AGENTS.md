# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, acquisition,
  progress, cancellation, supplemental-asset retention/purge, and the Input
  handoff under `src/app/remoteSource/`.
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- Import session-asset coordination from `src/ui/remoteSource`.
- Import acquire intents and the Input handoff result type from
  `src/app/remoteSource`.
- `index.ts` is the export surface. Do not import `workflow.ts`,
  `sessionAssets.ts`, or `state.ts` from outside this owner.

## Hard Invariants

- Closing the dialog does not cancel an in-flight acquisition. Polling and
  selected hidden titles survive ordinary close. App disposal and native
  cancel/purge remain the cleanup authorities.
- Acquisition poll patches publish through the Remote Source owner view.
  Native jobs provide a progress snapshot from job creation;
  `RemoteSourceAcquireView` renders its live percentage and Cancel.
  File List and the inspector subscribe to `subscribeRemoteSourceSupplementalAssets`
  so PDF chips update after Input has already published the new files.
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
  close-does-not-cancel, and publication of polled `getAcquisitionStatus`
  snapshots.
- `display.test.ts` pins terminal classification so polling cannot spin forever.
- `selection.test.ts` pins filter/selection policy.
- `RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent and
  the polled owner-to-Solid progress path.

## Breaking-Change Triggers

- Adding, removing, or renaming a public session-asset export.
- Dual-writing `fileListSessionState` or adding a parallel remote file list.
- Cancelling acquisition from dialog close.
