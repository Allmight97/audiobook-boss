# UI Foundation

## Current state

Shared visual behavior crosses `src/ui/foundation/index.ts`. Native CSS is the
only styling language. Next action: change tokens or primitives here.
ProductionRoot and the Vite mock-runtime are the visual references; do not
add a second catalog.

## Public API Strip

- Import from `src/ui/foundation`.
- Exports: `Button`, `CoverThumb`, `Dialog`, `Progress`, and their prop types.
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
- Theme follows `prefers-color-scheme`. Density follows
  `[data-density='compact']` on `html`. Do not add TypeScript theme or density
  props.

## Proof

- `bun run test -- src/ui/foundation`
- `bun run test -- scripts/frontend-toolchain-layout.test.ts`
