# Effect Route Card

- ABB packages: `effect`
- Upstream: `https://github.com/Effect-TS/effect.git`
- Context7: `/llmstxt/effect_website_llms_txt` or `/effect-ts/effect`

Resolve the installed version from `bun.lock` (do not cache it here).

## Use For

- Effect workflows, typed errors, service/context boundaries, layers, scopes,
  streams, schedules, schemas, and Effect test idioms.
- Idiomatic examples before designing ABB Effect owners or helpers.

## Installed / registry entrypoints

- `node_modules/effect/dist/index.d.ts`
- `node_modules/effect/dist/Effect.d.ts`
- `node_modules/effect/dist/Context.d.ts`
- `node_modules/effect/dist/Layer.d.ts`
- `node_modules/effect/dist/Scope.d.ts`
- `node_modules/effect/dist/Schema.d.ts`
- `node_modules/effect/dist/Stream.d.ts`
- `node_modules/effect/dist/Schedule.d.ts`
- npm/unpkg: `effect` at the lockfile version (`packages/effect` in the
  upstream monorepo)

## Exceptional upstream areas

- `packages/effect/src/Effect.ts` and sibling modules
- `packages/effect/test/Effect`, `Schema`, `Stream`, `Scope.test.ts`,
  `Schedule.test.ts`

## Avoid

- Import the installed `effect` package, not unpublished monorepo paths.
- Do not copy internal helpers from `src/internal` unless an exported installed
  API proves the same capability exists.
- Effect `llms.txt` is current documentation, not versioned evidence.

## ABB Reconciliation

- ABB is on Effect 4 (exact pin in `package.json`). Reconcile docs against
  `bun.lock`. Workflow owners import Effect only through
  `src/lib/effect/appEffect.ts`.
- Check `package.json` and `bun.lock` for the resolved `effect` version.
- Prefer installed TypeScript declarations and exported APIs over source-only
  internals.
- Keep Effect private to the owning ABB workflow until an explicit boundary
  decision says otherwise.
- For tests, verify whether ABB currently uses plain Vitest or `@effect/vitest`
  before copying upstream test style.
