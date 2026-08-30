# Solid Route Card

- ABB packages: `solid-js`, `vite-plugin-solid`, `@solidjs/testing-library`
- Upstreams:
  - `solid-js`: `https://github.com/solidjs/solid.git`
  - `vite-plugin-solid`: `https://github.com/solidjs/vite-plugin-solid.git`
  - `@solidjs/testing-library`:
    `https://github.com/solidjs/solid-testing-library.git`
- Context7: `/solidjs/solid` or `/websites/docs_solidjs_com`

Resolve installed versions from `bun.lock` (do not cache them here).

## Use For

- Solid rendering, signals, effects, and component tests.
- Verifying whether ABB UI behavior follows current Solid patterns.

## Installed / registry entrypoints

- `node_modules/solid-js/types/index.d.ts`
- npm/unpkg: `solid-js` at the lockfile version

## Avoid

- Do not treat upstream app/docs infrastructure as ABB UI architecture.
- Do not copy runtime internals into ABB components.
- Do not broaden diagnostics to alternate viewport behavior; ABB is desktop-only
  unless a task explicitly asks otherwise.
- Do not reintroduce `@effect/atom-solid` or `effect/unstable/reactivity`.

## ABB Reconciliation

- Check `package.json` and `bun.lock` for the resolved `solid-js` version.
- Verify owner conventions in the nearest `src/app/<owner>/index.ts` strip
  and Solid view before changing state shape.
- Views take runtime owners from Solid context.
- Use targeted frontend tests for deterministic behavior and browser/human
  review for visual UX claims.
