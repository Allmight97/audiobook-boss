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
  intents. `attachEngineLink` resolves once the engine's first snapshot is in
  the link, so every part an adapter reads is engine truth; a reply missing a
  part fails the attach. It listens before it attaches and applies what arrived
  in between on top of the reply, so no update is missed; part revisions make
  applying one twice harmless. WorkRuntime operations come with the attach
  reply and then as `work-operations-update` events, merged by
  `workOperations.ts` (newest snapshot per operation, newest order). A cancel's
  change arrives as one of those events; its reply carries nothing.
- `send` resolves with the intent's outcome after its work finishes. `post` is
  for intents whose outcome and completion nobody awaits; its refusal shows as
  the link's `refusal` (`RefusedChangeNotice`). A `send` caller with its own
  status line shows a rejection there (cover loads, processing, Remote
  Source); the metadata echo, lookup actions and file selection pass it to
  `link.refuse`, the same notice `post` uses. The host runs both in the order
  called; a gap in the numbering is skipped after `MISSING_INTENT_WAIT`
  (`src-tauri/AGENTS.md`).
- `send` itself sets the `refusal` for an outcome that changes nothing and shows
  nothing else: `draftRejected` with no message (a Save is running), and
  `editLost` for a field edit or cover load whose form was replaced.
  `superseded` stays silent, where a newer request replacing it is the design
  (a row click, a collision answer, a newer cover choice).
- A part that arrives unchanged keeps object identity for its files and lookup
  results. A changed remote part keeps the objects of its unchanged selection
  lists, PDF choices, releases, and grab records. Solid rows are not rebuilt,
  and a click does not land on a replaced element.
- Adapters show typed text (form fields, lookup queries, the naming template,
  the Remote Source Indexer URL) through `createTypingEcho`: at once, until the
  engine's reply for that keystroke arrives, and only in the scope the owner
  names (the form's `binding`, the lookup queue item). The echo is display only
  and never decides what is saved.
  Form edits send the `binding` the form shows.

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

- `createAppRuntime()` attaches the engine link, then creates one instance of
  every owner inside one Solid root, and disposes them together. It rejects
  when the attach fails. `AppRoot` owns that wait: it renders nothing until the
  runtime exists, the failure as an alert, and disposes a runtime that arrives
  after it unmounted. A late engine reply after disposal changes nothing
  visible; a caller that awaited it still receives it.
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
  send Settings intents only. `quittingAvailable` is display-only, read through
  `src/lib/platform` (macOS only): the Quit shortcut is macOS-only, and bindings
  and fixtures must be the same on every OS.
- `encoding` (session AGENTS): option lists, the codec family, and the bitrate
  shown come from the engine's `facts`, and `labels.ts` is the one table of
  format, intent, encoder, and bitrate-mode wording. `project.ts` disables an
  option because the facts say so. `editFor` turns a control value into an
  `AudioEdit`; a value outside the engine's lists sends nothing. `form(scope)`
  (defaults, one title, or a selection) gives a panel its view, refusal,
  `choose`, and summary line. Sample-rate
  and FAAC quality lists still come from the capabilities. What Auto resolves
  to comes from the engine's `autoSampleRate` and `autoChannels` readings on
  the title or the selection, never from source facts or a plan; a selection's
  are plain Auto while the field is mixed. Defaults describe future imports
  and name no source; existing titles change only through Apply App Settings. `selectionView` and `refusal` render engine facts and combine no
  titles.
- `inputSession` (session AGENTS): a title's chapters, duration, and size
  come from the engine's `titleTotals`; the combined size and Group as one
  title availability come from `totalSizeBytes` and `canGroup`. A source row's
  own count is `acceptedChapterCount`, the plan the file converts with. Why: a
  CUE plan replaces the embedded chapters. Views and the owner branch on a
  title's or source's engine `status` (`ready`, `failed`, `removedFinished`),
  never on `isValid` or `sourceRemoved`. The owner words a title's detail line
  (`detailsFor`) and its CUE notice (`cueNoticeFor`): a CUE button shows only
  for an action the CUE source lists. `view().selectedTitles` and
  `isSelected(file)` name the selection by title identity.
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
  render its snapshot; the status panel only counts titles by status for its
  chips. `resetToIdle` does nothing while `previewRun` is set, which tells a
  preview's own ending from a later submission's outcome (the engine drops a
  finished run at the next Start or Preview). `cancelPreview` carries the run
  identity and an optional child identity; whole-preview cancel needs no native
  job id, and disposal does not cancel. The artwork shows from the run's
  `cover` address once the engine offers one. A finished preview opens through
  `link.openPreview(runId)`, even when the requesting frontend is disposed
  before the reply (host side: `src-tauri/AGENTS.md`). Submit availability comes from
  `output.submissionInProgress`; preview Cancel availability comes from the
  run's `cancellable`, and a title's from its child `cancellable`.
  `preview()` sends no length: the engine owns the preview length and content,
  and `previewTooltip()` words `titles.previewLength`.
  The status line is one pure `statusOf(previewRun, submission, localError,
  concurrency)` in a memo; a failed send or preview open is the only local
  state and shows until the engine reports a new preview run or submission.
  A submission's error line shows over a preview's step line, because a failed
  preview also leaves a blocked submission.
- `remoteSource` (`crates/abb-engine/src/remote_source/AGENTS.md`): `open` and
  `selectLane` send only the lane intent. Connection editing sends write-only
  key, URL, and category intents; keys never come back from the engine, and
  only the URL's typing echo and the key's visual echo stay local. The engine
  draft holds the URL as typed and normalizes it only on Test and Save, so the
  form shows the engine's draft, a refused URL included, once its reply
  arrives. Close
  leaves accepted work running. Disposal invalidates pending
  authorization-browser replies. An authorization URL occurs only in the
  initiating intent outcome; snapshots never reopen a browser. The
  status line is a local refusal, else a running job's progress message, else
  the lane's engine line (`remote.audibleMessage`, `remote.indexer.message`);
  the engine orders every other state. `isBusy` is the engine's `remote.busy`;
  no busy rule lives here. Status stays with its originating provider, and a
  new request refusal shows even when an older job has a retained message.
  Filtering and sorting keep the engine's
  selection. Release rows key on the engine's `RemoteRelease.key`, including Grab and Retry; the Grab button reads `remote.indexer.canGrabSelection`.
  Sent means the provider accepted, not that the download finished. `view()`
  changes with every remote update, a download tick included; the other
  `RemoteSourceOwner` accessors change only with their own fact. Filtered and
  sorted lists are owner memos.
- `workOperations` (`crates/abb-engine/src/work_runtime/AGENTS.md`): WorkRuntime
  snapshots are the only progress source for accepted background operations.
  Terminal status comes from `abb_processing_core::classify_run_terminal`.
  Operations come from the link (Engine Link, above); the owner keeps only
  cancel-pending state and error wording. After `reset`, a late cancel or
  reveal reply changes nothing.
  `labels.ts` is the one table of stage, operation-status, title-status and
  kind wording, typed over the generated unions so a new engine value fails
  typecheck; the Work Center, the status panel and `processing` read it.
  `view().error` is `{ kind: 'cancel' | 'reveal', message }`; a later success
  of the same kind clears it.
  `phase === 'active'` (running or cancelling) decides which work shows
  motion, for the Work Center and the preview bar alike; `phase` is the
  engine's answer, so no view keeps its own status list.
  `view()` changes with every progress tick, and Work Center rows read it: no
  narrower accessor exists. Only whole-operation cancels track pending state
  (the Cancel button disables while one is in flight); repeating any cancel
  is harmless.

## Tests

Tests run against `src/test/fixtures/fakeEngine.ts`. It starts from the
recorded fresh session and settings, records intents, answers `applied`, and
appends imported files. It holds no product rule;
`check-known-mistakes.ts` rejects an `intent.kind` branch in it other than
`import`. Seed the engine's answer on the intent that asks for it (`respond`,
`respondSettings`, `respondPreviewOutput`, `seedForm`, `seedSettings`,
`change`). What the real engine answers to a
sequence of intents (outcomes and the changed snapshot parts) is proved by
`runtime/recordedWorkflows.contract.test.ts`. The recorder writes `audio` as
null in every step, because no replay reads it and it was most of the file;
`recordedFixtures.contract.test.ts` keeps each step's keys equal to
`session-fresh.json`'s;
a new engine-owned behavior a view depends on becomes a scenario in
`src-tauri/src/bin/export_bindings/scenarios.json`, not a rule in the fake.

Add App Runtime two-instance proof when isolation changes.

## Done

The owner has one source of truth, one Public API Strip, and one disposal path.
