# UI Foundation

## Scope

Shared visual behavior crosses `src/ui/foundation/index.ts`, its Public API
Strip. Views import from `src/ui/foundation`. Native CSS is the only styling
language.

## Rules

- `CoverImage` is the one way a cover is shown.
- Views read the public semantic tokens on `:root` in `internal/tokens.css`.
  Only `src/styles.css` imports `internal/foundation.css`. Callers use the
  primitives' props, never their private class names. Owner layout stays in the
  owner stylesheet.
- A primitive stays only if deleting it redistributes real behavior across
  owners.
- `SplitButton.disabled` disables both triggers and hides an open menu, so an
  engine refusal state leaves no second submission path.
- Theme follows `prefers-color-scheme`.
- `Dialog` `unmountWhenClosed` mounts content only while open or fading out;
  the fade lasts the backdrop's declared transition (`dialogs.css`), so a
  closed dialog's rows and progress do no work.
- `Progress` `active` marks work running now (running or cancelling); queued,
  finished, and idle bars stay still. An active bar shimmers across its filled
  part only, or, under reduced motion, shows still stripes. Fill, stripes, and
  track keep at least 3:1 contrast in both themes (`tokens.css`).
