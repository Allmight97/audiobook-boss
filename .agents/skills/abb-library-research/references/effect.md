# Effect Route Card

- Upstream: `https://github.com/Effect-TS/effect.git` on `main`
- Local subtree: `repos/effect`

Refresh:

```bash
git subtree pull --prefix=repos/effect https://github.com/Effect-TS/effect.git main --squash
```

## Use For

- Effect workflows, typed errors, service/context boundaries, layers, scopes,
  streams, schedules, schemas, and Effect test idioms.
- Idiomatic examples before designing ABB Effect owners or helpers.

## Start Here

- `repos/effect/packages/effect/src/Effect.ts`
- `repos/effect/packages/effect/src/Context.ts`
- `repos/effect/packages/effect/src/Layer.ts`
- `repos/effect/packages/effect/src/Scope.ts`
- `repos/effect/packages/effect/src/Schema.ts`
- `repos/effect/packages/effect/src/Stream.ts`
- `repos/effect/packages/effect/src/Schedule.ts`

## Examples And Tests

- `repos/effect/packages/effect/test/Effect`
- `repos/effect/packages/effect/test/Schema`
- `repos/effect/packages/effect/test/Stream`
- `repos/effect/packages/effect/test/Scope.test.ts`
- `repos/effect/packages/effect/test/Schedule.test.ts`

## Avoid

- Do not import from `repos/effect`; ABB imports from the installed `effect`
  package.
- Do not copy internal helpers from `src/internal` unless an exported installed
  API proves the same capability exists.
- Do not treat Effect v4 notes inside upstream files as ABB truth unless ABB has
  adopted that version.

## ABB Reconciliation

- Check `package.json` and `bun.lock` for the installed `effect` version.
- Prefer installed TypeScript declarations and exported APIs over source-only
  internals.
- Keep Effect private to the owning ABB workflow until an explicit boundary
  decision says otherwise.
- For tests, verify whether ABB currently uses plain Vitest or `@effect/vitest`
  before copying upstream test style.
