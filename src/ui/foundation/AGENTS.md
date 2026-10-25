# UI Foundation

## Scope

Shared visual behavior crosses `src/ui/foundation/index.ts`, its Public API
Strip. Views import from `src/ui/foundation`. Native CSS is the only styling
language. The exact export names are the hand-written `STRIPS` list in
`src/__tests__/public-api-strips.contract.test.ts`.

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
- Motion: an element is complete when still. Movement that only decorates
  (transitions, the progress shimmer) is declared inside
  `@media (prefers-reduced-motion: no-preference)`. An endless animation also
  runs only while its owner says work is active (`Progress` `active`). Animate
  `transform` and `opacity`, never layout size. Why: an endless animation
  repaints every frame even when idle; under software rendering (WebKitGTK
  without compositing, WSL) the idle shimmer cost 150% of a CPU core.
  `scripts/motion-policy.test.ts` enforces the endless-animation and
  layout-size parts for every stylesheet under `src/`.
- `Progress` takes a required `label` (its accessible name) and `active` for
  work running now; queued, finished, and idle bars stay still.

## Proof

`bun run test -- src/ui/foundation scripts/motion-policy.test.ts`
