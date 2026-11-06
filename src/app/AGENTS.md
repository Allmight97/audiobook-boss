# Frontend Application Owners

## Scope

`src/app/<owner>` modules give Solid views one interface per product area.
Views under `src/ui` render these owners and dispatch intent. They keep no
second copy of owner state.

## Engine Adapters

- Input, metadata, lookup, settings, encoding, output, preview, remote choices,
  and Work Center progress are Rust-owned facts. Adapters word typed statuses
  and send intents. Engine truth for input, metadata, lookup, encoding, output,
  and processing: `crates/abb-engine/src/session/AGENTS.md`.
- Adapters keep frontend lifetime and presentation resources: dialog
  disclosure, transient typing echo, visible filter and sort, and opening the
  authorization browser. These never decide accepted file work or terminal
  truth.

## Engine Link

- `engineLink` is the one connection to the engine. It keeps the newest copy of
  each snapshot part (titles, selection, metadata, lookup, audio, output,
  remote, remote library, remote acquisition, settings) by revision, and sends
  intents. Adapters read the link. `workOperations` is the exception: it reads
  WorkRuntime through `tauriClient`. The link listens before it attaches, so no
  update published in between is missed; part revisions make applying one twice
  harmless.
- `send` resolves with the intent's outcome after its work finishes. `post` is
  for intents whose outcome and completion nobody awaits; its refusal shows as
  the link's `refusal` (`RefusedChangeNotice`). A `send` caller with its own
  status line shows a rejection there (cover loads, processing, Remote
  Source); the metadata echo, lookup actions and file selection pass it to
  `link.refuse`, the same notice `post` uses. The host runs both in the order
  called.
- A part that arrives unchanged keeps object identity for its files and lookup
  results. A changed remote part keeps the objects of its unchanged selection
  lists, PDF choices, releases, and grab records. Solid rows are not rebuilt,
  and a click does not land on a replaced element.
- Adapters show typed text (form fields, lookup queries, the naming template)
  through `createTypingEcho`: at once, until the engine's reply for that
  keystroke arrives, and only in the scope the owner names (the form's
  `binding`, the lookup queue item). The echo is display only and never decides what is saved.
  Form edits send the `binding` the form shows.
  The Remote Source URL echo does not use it: it drops only once the engine's
  URL matches the trimmed text, or at once when an applied settings Reset
  discards the typed connection edits (below).

## Owner Interface

- Each owner's `index.ts` is its Public API Strip (Oxlint enforces root
  imports). Cross-owner coordination uses the other owner's strip, injected
  when App Runtime composes the owners.
- Expose a small `view()` or accessor surface plus semantic intents. A fact
  that changes often gets its own narrow accessor, so a row re-runs only when
  its own fact changes.
- Cross-owner integration tests use public owner intents. Owner-internal tests
  may import private modules to prove behavior at its cheapest stable boundary.
  Test-only needs stay out of the strip.

## State And Lifetime

- `createAppRuntime()` creates the engine link and one instance of every owner
  inside one Solid root and disposes them together. A late engine reply after
  disposal changes nothing visible; a caller that awaited it still receives it.
- Accepted background operation truth lives in WorkRuntime.

## Workflow And Failure Shape

- Error normalization and cancellation follow `src/lib/tauri/AGENTS.md`. Keep
  typed provider diagnostics and backend terminal verdicts.

## Owners

One bullet per owner with an invariant the code does not show; an owner
without a bullet has none. Engine paths name the owner of the truth.

- `appSettings` (`crates/abb-engine/src/app_settings/AGENTS.md`): the owner
  words durability from `saveError` and the reset notice from `recovery`,
  shows the concurrency view (Auto shows the capability's `autoEffective`, not
  the current fixed count), and holds dialog state. Settings intents go
  through `sendSettings` (their own numbered order); a dialog control's
  failure shows beside that control (`runDialogAction`). The session records
  audio and output defaults and announces them with `settings-update`; views
  send Settings intents only.
- `encoding` (session AGENTS): `project.ts` disables an option because the
  engine's facts say so. `editFor` turns a control value into an `AudioEdit`;
  a value no control offers sends nothing. What Auto resolves to comes from the
  title's engine plan, never from source facts; for a selection it comes from
  the engine's `autoSampleRate` and `autoChannels` readings, which are plain
  Auto while the field is mixed. Whether User Preference is offered comes from
  `facts.encodeOffered`, never from the format. Defaults describe future
  imports and name no source; existing titles change only through Apply App
  Settings. `selectionView` and `refusal` render engine facts and combine no
  titles.
- `inputSession` (session AGENTS): a title's chapters, duration, and size
  come from the engine's `titleTotals`; the combined size and Group as one
  title availability come from `totalSizeBytes` and `canGroup`. A source row's
  own count is `acceptedChapterCount`, the plan the file converts with. Why: a
  CUE plan replaces the embedded chapters.
- `metadataLookup` (session AGENTS): query text binds to its queued path and
  metadata `binding`. Advancing or rebinding drops the previous title's echo at
  once. Apply sends the lookup part revision the snapshot holds.
- `outputPlan` (session AGENTS): the owner words preview text per engine
  preview kind and the size estimate. Size estimates show beside each title in
  File List; the Output and Encoder panels show none. `collision` words
  `output.collisionReview` and answers only the question shown; disposal sends
  no intent.
- `processing` (session AGENTS): `owner.ts` sends intents and builds no payload;
  `submit.ts` words `output.submission`. `restartPrompt` renders the engine's one
  eligible question, and `restartOffers` feeds Work Center retry. Restart and
  Keep Location send the title and revision shown; teardown answers neither.
  Preview identity, progress, and terminal truth come from `output.previewRun`:
  render its snapshot and aggregate nothing. `cancelPreview` carries the run
  identity and an optional child identity; whole-preview cancel needs no native
  job id, and disposal does not cancel. The artwork shows from the run's
  `cover` address once the engine offers one. A finished preview opens through
  `link.openPreview(runId)`, even when the requesting frontend is disposed
  before the reply (host side: `src-tauri/AGENTS.md`). Submit and Cancel
  availability follow the engine preview state, including for a frontend that
  attaches to an active run. `preview()` sends no length: the engine owns the
  preview length and content. Each owner instance owns its status store and
  publisher.
- `remoteSource` (`crates/abb-engine/src/remote_source/AGENTS.md`): `open` and
  `selectLane` send only the lane intent. Connection editing sends write-only
  key, URL, and category intents; keys never come back from the engine, and
  only unconfirmed URL typing and the key's visual echo stay local. An applied
  settings Reset reaches it through the settings owner's `onAppliedReset`
  (wired in `runtime/index.ts`) and `discardIndexerConnectionEdits()`, so the
  form shows the engine's default connection. Close
  leaves accepted work running. Disposal invalidates pending
  authorization-browser replies. An authorization URL occurs only in the
  initiating intent outcome; snapshots never reopen a browser. Render
  `settled` and `handoff` facts and infer no terminal precedence. `isBusy` is
  the engine's `remote.busy`; no busy rule lives here. Status stays
  with its originating provider, and a new request refusal shows even when an
  older job has a retained message. A connected account message stays on the
  idle status line, including after the library loads, so an ephemeral sign-in
  stays visible. Filtering and sorting keep the engine's
  selection. Release rows key on `(indexerId, guid)`, including Grab and Retry.
  Sent means the provider accepted, not that the download finished. `view()`
  changes with every remote update, a download tick included; the other
  `RemoteSourceOwner` accessors change only with their own fact. Filtered and
  sorted lists are owner memos.
- `workOperations` (`crates/abb-engine/src/work_runtime/AGENTS.md`): WorkRuntime
  snapshots are the only progress source for accepted background operations.
  Terminal status comes from `abb_processing_core::classify_run_terminal`.
  `model.ts` applies two rules: keep the newest snapshot per operation, and show
  operations in the engine's order. `reset` invalidates pending responses.
  `isWorkRunning` decides which work shows motion (running or cancelling), for
  the Work Center and the preview bar alike.
  Only whole-operation cancels track pending state; title cancels are
  idempotent in the backend.

## Tests

Tests run against `src/test/fixtures/fakeEngine.ts`. It starts from the
recorded fresh session and settings, records intents, answers `applied`, and
appends imported files. It holds no product rule;
`check-known-mistakes.ts` rejects an `intent.kind` branch in it other than
`import`. Seed the engine's answer on the intent that asks for it (`respond`,
`respondSettings`, `seedForm`, `seedSettings`, `change`). What the real engine answers to a
sequence of intents is proved by `runtime/recordedWorkflows.contract.test.ts`;
a new engine-owned behavior a view depends on becomes a scenario in
`src-tauri/src/bin/export_bindings/scenarios.json`, not a rule in the fake.

Add App Runtime two-instance proof when isolation changes.

## Done

The owner has one source of truth, one Public API Strip, and one disposal path.
