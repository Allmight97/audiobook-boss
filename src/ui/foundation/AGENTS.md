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
  repaints every frame even when idle; in WebKitGTK under Xvfb with software
  rendering (arm64 Linux VM), the idle shimmer cost about 150% of a CPU core
  with compositing on and 5% with it off. `scripts/motion-policy.test.ts`
  enforces the endless-animation (motion preference and `[data-active]`) and
  layout-size parts for every stylesheet under `src/`.
- A scrolling result list (`.app-modal-results`) sets `contain: content`.
  Why: without it, a running shimmer elsewhere in the dialog made WebKit redo
  every row each frame (5,000 titles: 51 ms frames while scrolling, 17 ms with
  containment). `content-visibility: auto` measured slower for these rows.
- `Progress` takes a required `label` (its accessible name) and `active` for
  work running now; queued, finished, and idle bars stay still.

## Proof

`bun run test -- src/ui/foundation scripts/motion-policy.test.ts`
