# Solid

Read for Solid rendering, reactivity, disposal, or component-testing questions.

- Packages: `solid-js`, `vite-plugin-solid`, `@solidjs/testing-library`;
  resolve versions from this checkout's `bun.lock`.
- Installed declarations: `node_modules/solid-js/types/index.d.ts`; check
  package exports when another entrypoint is involved.
- Upstreams: [Solid](https://github.com/solidjs/solid),
  [Vite plugin](https://github.com/solidjs/vite-plugin-solid), and
  [testing library](https://github.com/solidjs/solid-testing-library).
- Optional Context7 hints: `/solidjs/solid` or `/websites/docs_solidjs_com`.

Root `AGENTS.md` owns the checkout's Solid-major constraint.
`src/app/AGENTS.md` owns session state and disposal; `src/AGENTS.md` owns
frontend scope. Reconcile upstream examples with those owners and the installed
public APIs before changing a view or owner interface.
