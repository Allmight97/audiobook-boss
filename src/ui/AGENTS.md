# UI Views

Each view under `src/ui/<view>/` renders its owner from `useAppRuntime()` and
dispatches intents. It keeps screen-local state only: disclosure, focus, and
typed input. Owner rules: `src/app/AGENTS.md`.

A view folder's `index.ts` is its Public API Strip (Oxlint enforces folder
imports). Files in one folder import each other relatively.

## Composition

- `App.tsx` composes the views and owns the global key bindings (Cmd+S saves
  metadata, Cmd+, opens Settings).
- A `<For>` over option, provider, title, release, operation, or job rows a
  snapshot rebuilds passes `keyed` a stable key (`option.value`,
  `title.titleId`, `release.key`, `operation.operationId`,
  `child.childJobId`), so each row keeps its DOM. Why: a recreated `<option>`
  resets its `<select>` to the first option, and a button rebuilt by a progress
  tick between press and release loses the click. File list, lookup, and source
  rows pass none: `engineLink` keeps their object identity (`src/app/AGENTS.md`).
- Host-OS wording (the Quit shortcut, the file manager name, the multi-select
  modifier) comes from `src/lib/platform`, which reads the user agent when
  asked. `check-known-mistakes.ts` rejects the user agent and `⌘Q` elsewhere.
  ABB builds for macOS and Linux only.
- A view opens an external link with `tauriClient.openUrl` only for a link the
  user clicked: a release's source page and the Opus help link.
- State a test reads is an attribute: `data-state` on a file row (the engine's
  title status) and on a dialog (`open` or `closed`), `data-status` on a job
  row and a Work Center status. Tests find elements by role and accessible
  name and read those attributes, never CSS classes.

## Styles

- Motion: an element is complete when still. `scripts/motion-policy.test.ts`
  enforces the rest. Why: an endless animation repaints every frame; `all` and
  layout properties make each state change animate layout.
- A long scrolling list sets `contain: content` (`.app-modal-results`). Why:
  without it, motion elsewhere in the dialog made WebKit redo every row.

## View Traps

One bullet per view. A view without a bullet has no trap beyond its owner.

- `appSettings`: Escape closes connection help before it closes Settings. The
  API key input is write-only. Reset needs a second activation and stays
  disabled while its save is pending.
- `collisionDialog`: the collision question remounts per `reviewId`, so a button
  answers the review it rendered. Overwrite Existing, Skip Existing, Keep
  Existing, and Restart ignore a repeat press (`event.detail > 1`); a
  double-click's second press would answer a re-review the user has not read.
  Closing the restart dialog (Escape or close) keeps the
  location. Teardown answers neither question. Both dialogs use the foundation
  `Dialog` (`src/lib/tauri/AGENTS.md`, native dialogs).
- `encoderPanel`: one `EncoderView` serves Settings defaults, one title, or a
  selection, by the `scope` it passes to `runtime.encoding.form`. Every element
  id carries a per-instance suffix, because several panels can be open; tests
  find controls by label or `data-testid`. A mixed selection shows Mixed; the
  user picks a common format before editing encoder fields. Encoder fields show
  only under User Preference.
- `fileList`: every selection-changing action sends an Input intent, so the
  engine's draft gate runs; only a file click awaits its outcome. Removal and
  selection pass the title's identity, never its position. The listbox owns
  keyboard handling; rows are not tab stops.
  `pointerReorder.ts` serves the outer list and `TitleSources`. A drag holds the
  pressed row's identity key, not its position, and finds positions again at
  release, so an import or removal mid-drag cannot move another title. It
  removes its listeners on dispose and on `pointercancel`. `AudioHandlingControl` renders
  in a portal that overlays the list without changing row height; hover or
  focus opens it, click pins it, and locking input closes it. A copied audio
  plan shows a check and an explicit copy label, not color alone.
- `fileImport`: picks and path imports go through Input `importIntent`.
  Native cover-art drops go to Metadata `applyCoverArtDrop`. The engine imports
  files the OS opens and files remote acquisition fetches.
- `metadataLookup`: the dialog is a decision surface, so result covers load
  eagerly (`CoverImage eager`). Covers load from the `cover` addresses the
  engine offers, through `coverSrc`; a provider-controlled URL never goes into
  a DOM attribute.
- `remoteSource`: one dialog switches the Audible and Indexer lanes. Acquisition
  progress shows in the Audible lane only. Plain click replaces the selection;
  Cmd or Ctrl-click toggles one release. Grab All counts selections hidden by
  the filter and shows the hidden count. Select, Grab, and details accessible
  names include the indexer, because mirrored releases share titles. A failed
  source-page link shows beside its release row. Rows and the progress block
  read the owner's narrow accessors, never `view()`, so a download tick re-runs
  no row. The progress block is not a live region; the
  dialog status line announces stage changes. The dialog passes
  `unmountWhenClosed`, so closed, it holds no rows; each list keeps its
  scroll position and returns to it once its rows render on reopen.
- `tagPreview`: with several titles selected, the TSOA row omits each title's
  source value. It can show blank where outputs keep their own.
- `statusPanel` and `workCenter` render title rows with the foundation
  `JobRow` and inject their own actions: the panel's Cancel sends the
  preview's `cancelPreview`, the Work Center's sends the operation's
  `workOperations.cancel`. Status classes are `is-<generated status value>`.
- `workCenter`: a title row offers Cancel only in a multi-title operation whose
  child is `cancellable` (the engine clears it once the title is
  cancelling). Restart and Keep Location show
  only when an engine restart offer matches both the operation and the input,
  so retained rows from older exports never get actions. The reveal button
  label names the host file manager.
