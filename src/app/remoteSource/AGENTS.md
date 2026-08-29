# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, acquisition,
  progress, cancellation, supplemental-asset retention/purge, and the Input
  handoff under `src/app/remoteSource/`.
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- Import session-asset coordination from `src/ui/remoteSource`.
- Import acquire intents, view atoms, and the Input handoff result type from
  `src/app/remoteSource`.
- `index.ts` is the export surface. Do not import `atoms.ts`, `workflow.ts`,
  `sessionAssets.ts`, or `state.ts` from outside this owner except existing
  processing tests that still call `removeRemoteSourceSupplementalAssets`.

## Hard Invariants

- Closing the dialog does not cancel an in-flight acquisition. Polling and
  selected hidden titles survive ordinary close. App disposal and native
  cancel/purge remain the cleanup authorities.
- `runRemoteSourceActionAtom` must publish `remoteSourceViewAtom` on each
  acquisition poll patch. Waiting until the Effect completes hides live
  `getAcquisitionStatus` progress from the Solid dialog.
- Materialized audio becomes a normal Input session through
  `importIntentAtom`. Do not call `handleImportedAudioPaths` or
  `getCurrentFileList`. If Input import is blocked or fails, purge the staged
  remote session immediately.
- Supplemental assets are keyed by the imported file `inputId`, not by
  provider path after handoff. Do not add a second file-list store.
- `remoteSourceLifetimeAtom` purges sessions for input ids that leave the
  public Input view. File Import must mount that atom so the subscription
  stays alive.
- Frontend state may hold provider-neutral account, title, job, and
  diagnostic text. It must not persist credentials, tokens, cookies, license
  material, or raw provider payloads.

## Testing

- `sessionAssets.test.ts` pins input-id rekey, companion summaries that omit
  paths, retainer deferral, and shared-job purge.
- `workflow.test.ts` pins successful Input handoff, blocked-import purge,
  close-does-not-cancel, and live `getAcquisitionStatus` download progress.
- `display.test.ts` pins terminal classification so polling cannot spin forever.
- `selection.test.ts` pins filter/selection policy.
- `RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent.

## Breaking-Change Triggers

- Adding, removing, or renaming a public session-asset export.
- Dual-writing `fileListSessionState` or adding a parallel remote file list.
- Cancelling acquisition from dialog close.
