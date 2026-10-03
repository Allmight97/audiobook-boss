# Remote Source

## Scope

- Adapts the engine's remote snapshot into the acquisition dialog and Indexer
  Settings. The engine owns title/PDF/release choices, connection drafts,
  accepted account/authentication/library state, searches and sequential Grab
  batches, acquisition progress, and handoff (`crates/abb-engine/src/remote_source/AGENTS.md`).
- Screen-local state holds dialog visibility, filters/sort, search text, auth
  handoff text, and cover thumbnail resources.
  Product acceptance rules go in the engine.

## Public API Strip

- `index.ts` exposes the composed `RemoteSourceOwner`; state,
  connection echo, and cover previews remain private.
- `open` and `selectLane` send only the lane intent; the engine refreshes
  account and library facts.
  Title/PDF/release selection, Search, Grab, Acquire, and Cancel send session
  intents through `engineLink`.
- Connection editing sends write-only key/URL/category intents. Only unconfirmed
  typing and the entered key's visual echo remain local; keys never come back
  from the engine. Save/Test status comes from the engine snapshot.
- Input companion summaries read validated assets from engine titles.

## Lifetime And Display

- Close leaves accepted work running. Disposal invalidates local authorization
  browser-opening replies and thumbnails, while a replacement frontend reads
  current remote account/auth/library facts, progress and choices from its
  session attachment.
- Render acquisition `settled` and `handoff` facts. Do not infer terminal
  precedence or wait on a separate acquisition event stream. The engine imports
  and removes staged files; the frontend only words the outcome.
- Status stays with its originating provider. Display a new request refusal
  even when an older job or search has a retained message.
- Visible filtering/sorting preserves engine selection. Release rows use the
  `(indexerId, guid)` pair, including for Grab and Retry. Sent describes provider
  acceptance, not download completion.
- Preserve per-instance lifetime for authorization browser opening, password
  echo, and thumbnail resources. None may publish into a replacement owner.

- Authorization URLs occur only in the initiating intent outcome; snapshots
  never reopen a browser.

## Proof

- `workflow.test.ts` covers intent routing, snapshot reattachment, handoff wording,
  auth browser opening, restored library rows, and refusal visibility.
  `engineLink/link.test.ts` guards independent library revisions.
  `indexerConnection.test.ts` covers engine draft rendering and write-only input echo.
- `selection.test.ts` proves visible filter/sort; selection and batch rules are
  proved by engine `remote_source/ui_tests.rs`.
- `RemoteSourceAcquireView.test.tsx` covers dialog close, progress/Cancel,
  Indexer identity, failure Retry, and search interaction.
- Add two-runtime proof when ownership or disposal semantics change.

## Boundary Changes

- Exposing private state mutation or reconstructing engine policy.
- Deciding when staged downloads are kept/removed or cancelling on Close.
