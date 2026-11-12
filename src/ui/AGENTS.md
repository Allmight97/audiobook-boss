# UI Views

- A view keeps only screen-local state (disclosure, focus, typed input); every
  other value comes from its owner, and every change is an intent.
- A `<For>` over rows a snapshot rebuilds passes `keyed` a stable key, because
  a recreated `<option>` resets its `<select>` to the first option and a
  button rebuilt by a progress tick between press and release loses the click.
  File list, lookup, and source rows pass none: `engineLink` keeps their
  object identity.
- A view opens an external link with `tauriClient.openUrl` only for a link the
  user clicked.
- A long scrolling list sets `contain: content` (`.app-modal-results`), because
  without it motion elsewhere in the dialog made WebKit redo every row.
- Motion: an element is complete when still, because an endless animation
  repaints every frame and `all` or layout properties animate layout on each
  state change (`scripts/motion-policy.test.ts`).

## View traps

One bullet per view; a view without one has no trap beyond its owner.

- `collisionDialog`: Overwrite Existing, Skip Existing, Keep Existing, and
  Restart ignore a repeat press (`event.detail > 1`), because a double-click's
  second press would answer a re-review the user has not read. Closing the
  restart dialog keeps the location, and teardown answers neither question:
  teardown is not a user cancellation.
- `encoderPanel`: every element id carries a per-instance suffix, because
  several panels can be open at once.
- `fileList`: every selection change sends an Input intent so the engine's
  draft gate runs, and removal and selection pass the title's identity, never
  its position. A drag holds the pressed row's identity and finds positions
  again at release, so an import or removal mid-drag cannot move another
  title. A copied audio plan shows a check and an explicit copy label, not
  color alone.
- `metadataLookup`: result covers load eagerly, because the dialog is a
  decision surface. A provider-controlled URL never goes into a DOM attribute;
  covers load from the engine's `cover` addresses through `coverSrc`.
- `remoteSource`: Select, Grab, and details accessible names include the
  indexer, because mirrored releases share titles. Rows and the progress block
  read the owner's narrow accessors, never `view()`, so a download tick
  re-runs no row.
- `tagPreview`: with several titles selected, the TSOA row omits each title's
  source value, so it can show blank where outputs keep their own.
- `workCenter`: Restart and Keep Location show only when an engine restart
  offer matches both the operation and the input, so retained rows from older
  exports never get actions.

## Foundation (`src/ui/foundation`)

- A select whose value the engine owns uses `Select`: it shows the owner's
  value again after sending a choice, because a refusal changes nothing that
  would reset the element.
- `JobRow` keys injected `actions` by `id`, so a progress tick does not
  rebuild a button between press and release.
- Open modals form one stack (`src/ui/foundation/internal/modal.ts`) and
  only the topmost handles Escape and Tab, because each controller listens on
  `document` and `stopPropagation` cannot order listeners on one node.
- Callers use a primitive's props, never its private class names; the
  `.app-modal-*` dialog-body classes in
  `src/ui/foundation/internal/dialogs.css` are the shared exception. No
  check enforces this.
- `Progress` fill, stripes, and track keep at least 3:1 contrast in both
  themes (`src/ui/foundation/internal/tokens.css`).
- A primitive stays only if deleting it would spread real behavior across
  owners.
