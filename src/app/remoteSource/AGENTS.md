# Remote Source

## Scope

- Owns account/auth display state, library scan, selection, starting and
  cancelling acquisitions, and showing their progress under
  `src/app/remoteSource/`. The engine imports a finished acquisition's files
  into the session, keeps its downloads while anything needs them, and
  removes them (`crates/abb-engine/src/session/AGENTS.md`).
- Solid dialog lives in `src/ui/remoteSource/RemoteSourceAcquireView.tsx`. It
  renders this owner; it does not keep a second acquisition store.

## Public API Strip

- `index.ts` is the export surface. Callers consume the composed
  `RemoteSourceOwner`; private state, workflow, and cover previews stay here.
- `open({ lane? })` and `selectLane` share workflow-owned entry: refresh account
  state and load the connected Audible library, without a mounted view. When
  an acquisition job exists, entry retains its status and library; Refresh rescans.
  `selectLane`, title/PDF/release selection intents, and workflow actions own
  transitions. `editSearch` accepts only user-editable search/filter/sort fields;
  connection edits accept only URL, API-key, and category drafts.
- State, workflow generations, and cover previews belong to each owner
  instance. Reset/disposal invalidates that instance's work.
- Companion PDFs are title facts: Input's `hasCompanions` and
  `companionSummary` read them from the engine's titles part.

## Hard Invariants

- Closing the dialog does not cancel an in-flight acquisition. Following it and
  selected hidden titles survive ordinary close. Lane switches reset Indexer
  results and Audible selection UI only; they do not cancel in-flight Audible
  acquisition. Only explicit cancel stops one; the engine removes downloads.
- Status text belongs to its originating provider and the view projects the
  selected provider's message. Background Audible progress, terminal outcomes,
  and errors cannot replace Indexer status or clear its pending-operation busy state.
  An acquisition owns Audible busy state until its handoff settles or cancellation
  succeeds; source reentry cannot enable a competing acquisition.
- An acquisition follows the engine's `acquisition-update` events after one
  status read to catch up; it settles when the job fails, is cancelled, or
  carries its `handoff`. `RemoteSourceAcquireView` renders the live percentage
  and Cancel. Cancellation and app disposal invalidate the active acquisition
  generation and wake its wait, so late events cannot overwrite terminal or
  reset state.
- The engine's `handoff` says how many titles were imported or why the files
  were removed; this owner only words it. It never imports, registers, or
  removes downloads.
- Indexer sort is session state: most seeders by default, or largest size with
  seeders as the tie-breaker. Filtering and sorting preserve release selection.
- Release selection and per-release Grab outcomes use `releaseKey` for the
  `(indexerId, guid)` pair; GUID alone is not unique across indexers.
  `selectRelease` replaces selection or toggles one item with the multi option.
  Row Grab and Grab All share the same sequential submission workflow, skip
  already-sent releases, and retain individual failures for explicit retry.
- Indexer selection and outcomes survive filtering, sorting, and same-lane
  close/reopen. A fresh search or lane switch clears them. An in-flight Grab
  batch captures its releases and owns the busy state until settled; reopening
  cannot hydrate over that state or replace its lane. Pending searches and
  account loads allow an explicit cross-lane reopen; their late results expire. Reset invalidates late
  responses and stops sending remaining items.
- Connection Save and Grab batches are mutually exclusive, including when
  settings drafts change during a pending save. The engine refuses either
  while the other runs and never resends a release within one search; the
  checks here keep the dialog from offering what the engine would refuse. A save expires previous Indexer
  results and searches; submissions must come from a fresh search.
- Grab queues externally and never calls the Input handoff. Sent means the
  configured provider confirmed submission; it is not download-completion truth.
- Frontend state may hold provider-neutral account, title, job, and
  diagnostic text. It must not persist credentials, tokens, cookies, license
  material, or raw provider payloads.

## Testing

- `workflow.test.ts` pins handoff wording, progress from events, close does
  not cancel, a late event cannot overwrite cancellation, lane switch without
  cancelling Audible jobs, Indexer grab success/failure, unconfigured Indexer
  hydrate, and same-lane reopen preservation with cross-lane reset.
- `indexerConnection.test.ts` pins draft-only Test, write-only key behavior, successful
  Save refreshing open Indexer account state, and delayed loading preserving edits.
- `display.test.ts` pins terminal classification.
- `selection.test.ts` pins filter/selection policy and Indexer release seeder
  order.
- `src/ui/remoteSource/RemoteSourceAcquireView.test.tsx` pins Escape/Close to the close intent,
  the event-driven owner-to-Solid progress path, Indexer release protocol/category
  tags, and Enter-to-search on the author and title fields.
- When lifetime ownership changes, add two-runtime proof covering the affected
  state or resource: disposing A cannot cancel, reset, or publish into B.

## Boundary Changes

- Exposing internal state mutation.
- Deciding here when a download is kept or removed.
- Cancelling acquisition from dialog close.
