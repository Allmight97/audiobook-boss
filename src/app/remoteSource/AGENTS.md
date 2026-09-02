# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, acquisition,
  progress, cancellation, supplemental-asset retention/purge, and the Input
  handoff under `src/app/remoteSource/`.
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- Import `createRemoteSourceOwner`, owner types, and pure display/selection
  policy from `src/app/remoteSource`.
- `index.ts` is the export surface. `workflow.ts`, `sessionAssets.ts`,
  `coverPreview.ts`, and `state.ts` are private implementation modules.
- Each `RemoteSourceOwner` instance owns its state, workflow generations,
  cover-preview scheduler/cache, Supplemental Asset maps, and purge
  coordination. Do not add module-global compatibility state or raw
  retain/release/purge exports.

## Hard Invariants

- Closing the dialog does not cancel an in-flight acquisition. Polling and
  selected hidden titles survive ordinary close. App disposal and native
  cancel/purge remain the cleanup authorities.
- Acquisition poll patches publish through the Remote Source owner view.
  Native jobs provide a progress snapshot from job creation;
  `RemoteSourceAcquireView` renders its live percentage and Cancel.
  File List and the inspector observe Supplemental Assets through the composed
  Remote Source owner so PDF chips update after Input has already published the
  new files.
  Cancellation and app disposal invalidate the active acquisition generation
  so late Promise completions cannot overwrite terminal or reset state.
- Materialized audio becomes a normal Input session through
  Input `importIntent`. Do not call `handleImportedAudioPaths` or
  `getCurrentFileList`. If Input import is blocked or fails, purge the staged
  remote session immediately.
- Supplemental assets are keyed by the imported file `inputId`, not by
  provider path after handoff. Processing uses `processingAssets` and
  `withSubmissionRetention`; Work Operations reports terminal Input facts with
  `settleTerminalWork`. Callers do not sequence raw retain/release/purge.
- Remote Source purges sessions for input ids that leave the public Input
  view. File Import keeps that lifetime subscription alive.
- Frontend state may hold provider-neutral account, title, job, and
  diagnostic text. It must not persist credentials, tokens, cookies, license
  material, or raw provider payloads.

## Testing

- `sessionAssets.test.ts` pins input-id rekey, companion summaries that omit
  paths, retainer deferral, shared-job purge, cleanup failure, and isolation.
- `workflow.test.ts` pins successful Input handoff, blocked-import purge,
  close-does-not-cancel, and publication of polled `getAcquisitionStatus`
  snapshots through owner instances, plus owner and cover-cache isolation.
- `display.test.ts` pins terminal classification so polling cannot spin forever.
- `selection.test.ts` pins filter/selection policy.
- `RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent and
  the polled owner-to-Solid progress path.
- When lifetime ownership changes, add two-runtime proof covering the affected
  state or resource: disposing A cannot cancel, purge, reset, or publish into B.

## Breaking-Change Triggers

- Exporting raw session-asset, workflow-state, cache, or listener controls.
- Dual-writing `fileListSessionState` or adding a parallel remote file list.
- Cancelling acquisition from dialog close.
