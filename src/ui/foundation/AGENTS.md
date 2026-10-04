# UI Foundation

## Scope

Shared visual behavior crosses `src/ui/foundation/index.ts`. Native CSS is the
only styling language.

## Public API Strip

- Import from `src/ui/foundation`.
- Exports: `Button`, `CoverImage`, `CoverThumb`, `Dialog`, `Progress`, `SplitButton`, and their prop types.
- `CoverImage` is the one way a cover is shown: its container is the
  placeholder while it loads, it fades in, it loads lazily unless `eager`, and
  a failed load shows a state, never a broken image.
- Public semantic tokens live on `:root` in `internal/tokens.css`. Owner CSS
  may consume those custom properties. It may not import this private cluster.

## Private Cluster

- Solid primitives, `internal/modal.ts`, and the ordered CSS entry
  `internal/foundation.css`.
- Callers do not import private class names as a second authoring language.
  Owner layout stays in the owner stylesheet.

## Invariants

- No Tailwind, CSS-in-JS, CSS Modules, token generators, or component kits.
- No `sx`, style-object, or public utility catalog.
- A primitive stays only if deleting it redistributes real behavior across
  owners. Field and Surface fail that test today.
- `SplitButton.disabled` disables both triggers and hides an open menu so an
  engine refusal state cannot leave a second submission path available.
- Theme follows `prefers-color-scheme`. Do not add TypeScript theme props.

## Proof

- `bun run test -- src/ui/foundation`
- `bun run test -- scripts/frontend-toolchain-layout.test.ts`
