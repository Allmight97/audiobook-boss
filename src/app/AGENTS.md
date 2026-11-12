# Frontend Application Owners

- Input, metadata, lookup, settings, encoding, output, preview, remote
  choices, and Work Center progress are engine facts. An owner words typed
  statuses and sends intents; views keep no second copy of owner state.
- An owner keeps only frontend lifetime and presentation resources: dialog
  disclosure, typing echo, visible filter and sort, opening the authorization
  browser. None of them decides accepted file work or terminal truth.
- `attachEngineLink` listens before it attaches and applies what arrived in
  between on top of the reply, so no update is missed; part revisions make
  applying one twice harmless.
- `send` sets the link's `refusal` for an outcome that changes nothing and
  shows nothing else (`draftRejected` with no message, `editLost`).
  `superseded` stays silent, because a newer request replacing it is the
  design.
- A part that arrives unchanged keeps the object identity of its rows, so
  Solid rows are not rebuilt and a click does not land on a replaced element.
- Expose a small `view()` or accessor surface plus semantic intents. A fact
  that changes often gets its own narrow accessor, so a row re-runs only when
  its own fact changes.
- Cross-owner integration tests use public owner intents. Owner-internal tests
  may import private modules to prove behavior at the cheapest stable
  boundary; test-only needs stay out of the owner's `index.ts`.
- Add an App Runtime two-instance test when isolation changes.
- Done means the owner has one source of truth, one `index.ts` surface, and
  one disposal path.

## Owners

One bullet per owner with a reason the code does not show; an owner without a
bullet has none.

- `appSettings`: `quittingAvailable` reads `src/lib/platform`, because the
  Quit shortcut is macOS-only while bindings and fixtures must match on every
  OS.
- `encoding`: what Auto resolves to comes from the engine's `autoSampleRate`
  and `autoChannels`, never from source facts or a plan; the owner combines no
  titles.
- `inputSession`: a source row shows `acceptedChapterCount`, the plan the file
  converts with, because a CUE plan replaces the embedded chapters.
- `processing`: `preview()` sends no length, because the engine owns preview
  length and content. A submission's error line shows over a preview's step
  line, because a failed preview also leaves a blocked submission.
- `remoteSource`: the import menu, Settings radios, and Acquire dialog read
  `lanes()` and `lane()` and never test account state themselves. An applied
  settings Reset clears the URL echo, an unaccepted key, and a kept refusal,
  because Reset replaces the draft and neither local fact has a snapshot
  transition that drops it.
- `workOperations`: `phase === 'active'` decides which work shows motion; no
  view keeps its own status list.

## Tests

- The fake engine (`src/test/fixtures/fakeEngine.ts`) holds no product rule:
  seed the engine's answer on the intent that asks for it. A new engine
  behavior a view depends on becomes a scenario in
  `src-tauri/src/bin/export_bindings/scenarios.json`.
- The recorder writes `audio` as null in every step, because no replay reads
  it and it would be most of the file.
