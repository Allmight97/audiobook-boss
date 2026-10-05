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
- Style with native CSS files and the public tokens. Biome rejects Tailwind and
  `foundation/internal` imports.
- A primitive stays only if deleting it redistributes real behavior across
  owners.
- `SplitButton.disabled` disables both triggers and hides an open menu, so an
  engine refusal state leaves no second submission path.
- Theme follows `prefers-color-scheme`.

## Proof

`bun run test -- src/ui/foundation`
