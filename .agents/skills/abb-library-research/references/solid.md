# Solid

Read for Solid rendering, reactivity, disposal, or component-testing questions.

- Packages: `solid-js`, `@solidjs/web`, `@solidjs/vite-plugin`,
  `@solidjs/testing-library`; resolve versions from this checkout's `bun.lock`.
- Installed declarations: `node_modules/solid-js/types/index.d.ts` and
  `node_modules/@solidjs/web/types/index.d.ts`; check package exports when
  another entrypoint is involved.
- Upstreams: [Solid](https://github.com/solidjs/solid),
  [Vite plugin](https://github.com/solidjs/solid-vite-plugin), and
  [testing library](https://github.com/solidjs/solid-testing-library).
- Optional Context7 hints: `/solidjs/solid` or `/websites/docs_solidjs_com`.

The root `AGENTS.md` (`src/` line) owns the checkout's Solid-major constraint.
`src/app/AGENTS.md` owns frontend owner state and disposal (the engine owns
the working session). Reconcile upstream examples with those owners and the installed
public APIs before changing a view or owner interface.
