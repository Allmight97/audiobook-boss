# Effect

Read for Effect API, typed failure, dependency, scope, or cancellation questions.

- Package: `effect`; resolve its version from `bun.lock`.
- Installed declarations: `node_modules/effect/dist/<Module>.d.ts`, including
  `Effect`, `Context`, `Layer`, `Scope`, and `Schema`.
- Upstream: [Effect](https://github.com/Effect-TS/effect); package sources and
  tests live under `packages/effect`.
- Optional Context7 hints: `/llmstxt/effect_website_llms_txt` or
  `/effect-ts/effect`. Current `llms.txt` guidance is not version proof.

ABB's workflow import and lifetime conventions are owned by
`src/lib/effect/AGENTS.md`; read that owner when applying research to a
workflow. Check the installed package's exported surface before adopting an
upstream idiom, especially across Effect major versions. For test examples,
check ABB's live test dependencies before assuming `@effect/vitest` is used.
