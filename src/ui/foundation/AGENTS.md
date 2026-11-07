# UI Foundation

## Scope

Shared visual behavior crosses `src/ui/foundation/index.ts`, its Public API
Strip. Views import from `src/ui/foundation`. Native CSS is the only styling
language.

## Rules

- `CoverImage` is the one way a cover is shown (`check-known-mistakes.ts`
  rejects `<img>` elsewhere).
- Views read the public semantic tokens on `:root` in `internal/tokens.css`.
  Only `src/styles.css` imports `internal/foundation.css`. Callers use the
  primitives' props, never their private class names. Owner layout stays in the
  owner stylesheet. The `.app-modal-*` classes in `internal/dialogs.css` are
  shared dialog-body layout that the lookup, collision, and remote dialogs use
  by name.
- A select whose value the engine owns uses `Select`: it sends the choice,
  then shows the owner's value again, so a refused choice never leaves the
  element showing it. Why: the owner's value does not change on a refusal, so
  nothing else resets the element.
- `JobRow` is the one title row for the status panel and the Work Center. The
  caller injects `actions`; the row keys them by `id`, so a progress tick does
  not rebuild a button between press and release.
- A primitive stays only if deleting it redistributes real behavior across
  owners.
- Theme follows `prefers-color-scheme`.
- `Dialog` `unmountWhenClosed` mounts content only while open or fading out;
  the fade lasts the backdrop's declared transition (`dialogs.css`), so a
  closed dialog's rows and progress do no work.
- Open modals form one stack (`internal/modal.ts`): only the topmost handles
  Escape and Tab, because each controller listens on `document` and
  `stopPropagation` cannot order listeners on one node.
- `Progress` `active` marks work running now (running or cancelling); queued,
  finished, and idle bars stay still. An active bar shimmers across its filled
  part only, or, under reduced motion, shows still stripes. Fill, stripes, and
  track keep at least 3:1 contrast in both themes (`tokens.css`).
