# Svelte Route Card

- ABB packages: `svelte`, `@sveltejs/vite-plugin-svelte`
- Upstream: `https://github.com/sveltejs/svelte.git`
- Context7: `/sveltejs/svelte` or `/websites/svelte_dev`

Resolve installed versions from `bun.lock` (do not cache them here).

## Use For

- Svelte 5 reactivity, runes, compiler behavior, generated output, component
  tests, stores, transitions, and runtime edge cases.
- Verifying whether ABB UI behavior follows current Svelte patterns.

## Installed / registry entrypoints

- `node_modules/svelte/types/index.d.ts`
- `node_modules/svelte/compiler`
- `node_modules/svelte/src/reactivity`
- `node_modules/svelte/src/store`
- npm/unpkg: `svelte` at the lockfile version

## Exceptional upstream areas

- `documentation/docs`
- `packages/svelte/src/internal/client`
- `packages/svelte/src/compiler`
- `packages/svelte/tests/runtime-runes`, `runtime-browser`, `compiler-errors`,
  `types`, `print`

## Avoid

- Do not treat upstream app/docs infrastructure as ABB UI architecture.
- Do not copy runtime internals into ABB components.
- Do not broaden diagnostics to alternate viewport behavior; ABB is desktop-only
  unless a task explicitly asks otherwise.

## ABB Reconciliation

- Check `package.json` and `bun.lock` for the resolved `svelte` and
  `@sveltejs/vite-plugin-svelte` versions.
- Verify ABB state conventions in `src/ui/**/state.svelte.ts` before changing
  component state shape.
- Use targeted frontend tests for deterministic behavior and browser/human
  review for visual UX claims.
