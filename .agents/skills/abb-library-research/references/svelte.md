# Svelte Route Card

- Upstream: `https://github.com/sveltejs/svelte.git` on `main`
- Local subtree: `repos/svelte`

Refresh:

```bash
git subtree pull --prefix=repos/svelte https://github.com/sveltejs/svelte.git main --squash
```

## Use For

- Svelte 5 reactivity, runes, compiler behavior, generated output, component
  tests, stores, transitions, and runtime edge cases.
- Verifying whether ABB UI behavior follows current Svelte patterns.

## Start Here

- `repos/svelte/documentation/docs`
- `repos/svelte/packages/svelte/src/reactivity`
- `repos/svelte/packages/svelte/src/internal/client`
- `repos/svelte/packages/svelte/src/compiler`
- `repos/svelte/packages/svelte/src/store`
- `repos/svelte/packages/svelte/types`

## Examples And Tests

- `repos/svelte/packages/svelte/tests/runtime-runes`
- `repos/svelte/packages/svelte/tests/runtime-browser`
- `repos/svelte/packages/svelte/tests/compiler-errors`
- `repos/svelte/packages/svelte/tests/types`
- `repos/svelte/packages/svelte/tests/print`

## Avoid

- Do not treat upstream app/docs infrastructure as ABB UI architecture.
- Do not copy runtime internals into ABB components.
- Do not broaden diagnostics to alternate viewport behavior; ABB is desktop-only
  unless a task explicitly asks otherwise.

## ABB Reconciliation

- Check `package.json` and `bun.lock` for the installed `svelte` and
  `@sveltejs/vite-plugin-svelte` versions.
- Verify ABB state conventions in `src/ui/**/state.svelte.ts` before changing
  component state shape.
- Use targeted frontend tests for deterministic behavior and browser/human
  review for visual UX claims.
