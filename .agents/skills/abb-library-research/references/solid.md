# Solid Route Card

- ABB packages: `solid-js`, `@effect/atom-solid`, `vite-plugin-solid`,
  `@solidjs/testing-library`
- Upstreams:
  - `solid-js`: `https://github.com/solidjs/solid.git`
  - `@effect/atom-solid`: `https://github.com/Effect-TS/effect.git`
    (`packages/atom/solid`)
  - `vite-plugin-solid`: `https://github.com/solidjs/vite-plugin-solid.git`
  - `@solidjs/testing-library`:
    `https://github.com/solidjs/solid-testing-library.git`
- Context7: `/solidjs/solid` or `/websites/docs_solidjs_com`

Resolve installed versions from `bun.lock` (do not cache them here).

## Use For

- Solid rendering, signals, effects, and component tests.
- Effect Atom Solid bindings (`useAtomValue`, `useAtomSet`) and the
  `@effect/atom-solid` peer range against `solid-js`.
- Verifying whether ABB UI behavior follows current Solid patterns.

## Installed / registry entrypoints

- `node_modules/solid-js/types/index.d.ts`
- `node_modules/@effect/atom-solid`
- npm/unpkg: `solid-js` and `@effect/atom-solid` at the lockfile versions

## Avoid

- Do not treat upstream app/docs infrastructure as ABB UI architecture.
- Do not copy runtime internals into ABB components.
- Do not broaden diagnostics to alternate viewport behavior; ABB is desktop-only
  unless a task explicitly asks otherwise.

## ABB Reconciliation

- Check `package.json` and `bun.lock` for the resolved `solid-js` and
  `@effect/atom-solid` versions.
- Verify owner conventions in the nearest `src/app/<owner>/index.ts` strip
  and Solid view before changing state shape.
- Keep direct `effect/unstable/reactivity` and `@effect/atom-solid` imports in
  `src/app/runtime/reactivity.ts` and `src/app/runtime/solid.ts`.
- Use targeted frontend tests for deterministic behavior and browser/human
  review for visual UX claims.
