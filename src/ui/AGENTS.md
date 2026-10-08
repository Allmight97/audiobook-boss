# UI Views

Each view under `src/ui/<view>/` renders its owner from `useAppRuntime()` and
dispatches intents. It keeps screen-local state only: disclosure, focus, and
typed input. Owner rules: `src/app/AGENTS.md`.

A view folder's `index.ts` is its Public API Strip. Import another view through
`src/ui/<view>`; Biome rejects deeper paths. Files in one folder import each
other relatively. The exact export names are the hand-written `STRIPS` list in
`src/__tests__/public-api-strips.contract.test.ts`.

## Composition

- `App.tsx` composes the views and owns the global key bindings (Cmd+S saves
  metadata, Cmd+, opens Settings).
- `metadataManager/` arranges the metadata form and cover art views.
  `outputAndTags.css` holds the Output and tags panel layout.
- Left column: the input workflow flexes and the inspector
  (`leftColumn/FileInspectorView`) stays pinned.

## View Traps

One bullet per view. A view without a bullet has no trap beyond its owner.

- `appSettings`: Escape closes connection help before it closes Settings. The
  API key input is write-only. Reset needs a second activation and stays
  disabled while a save is pending.
- `collisionDialog`: the collision question remounts per `reviewId`, so a button
  answers the review it rendered. Destructive answers ignore a repeat press
  (`event.detail > 1`); a double-click's second press would answer a re-review
  the user has not read. Closing the restart dialog (Escape or close) keeps the
  location. Teardown answers neither question. Both dialogs use the foundation
  `Dialog`, because an OS prompt cannot be dismissed when a frontend is
  replaced.
- `encoderPanel`: one `EncoderView` serves Settings defaults (no props), one
  title (`title`), or a selection (`titles`). A mixed selection shows Mixed; the
  user picks a common format before editing encoder fields. Encoder fields show
  only under User Preference.
- `fileList`: every selection-changing action awaits an Input intent so the
  engine's draft gate runs. Removal passes the title's identity, never its
  position. The listbox owns keyboard handling; rows are not tab stops.
  `pointerReorder.ts` serves the outer list and `TitleSources`, and removes its
  listeners on dispose and on `pointercancel`. `AudioHandlingControl` renders
  in a portal that overlays the list without changing row height; hover or
  focus opens it, click pins it, and locking input closes it. A copied audio
  plan shows a check and an explicit copy label, not color alone.
- `fileImport`: picks and path imports go through Input `importIntent`.
  Native cover-art drops go to Metadata `applyCoverArtDrop`. The engine imports
  files the OS opens and files remote acquisition fetches.
- `metadataLookup`: the dialog is a decision surface, so result covers load
  eagerly (`CoverImage eager`). Covers load from `coverSrc` addresses; a
  provider-controlled URL never goes into a DOM attribute.
- `remoteSource`: one dialog switches the Audible and Indexer lanes. Acquisition
  progress shows in the Audible lane only. Plain click replaces the selection;
  Cmd or Ctrl-click toggles one release. Grab All counts selections hidden by
  the filter and shows the hidden count. Select, Grab, and details accessible
  names include the indexer, because mirrored releases share titles.
- `tagPreview`: with several titles selected, the TSOA row omits each title's
  source value. It can show blank where outputs keep their own.
- `workCenter`: a title row offers Cancel only in a multi-title operation whose
  child is `cancellable` and not yet cancelling. Restart and Keep Location show
  only when an engine restart offer matches both the operation and the input,
  so retained rows from older exports never get actions. The reveal button
  label names the host file manager.
