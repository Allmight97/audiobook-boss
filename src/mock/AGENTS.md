# Browser Mock Runtime

## Scope

Vite-only in-browser stand-in for Rust so the real `ProductionRoot` can be
clicked through without a Tauri window. Official mocks intercept invoke and
events. Views still call `tauriClient`.

## Public surface

- Command: `bun run ui:mock` (`ABB_UI_MOCK=1 vite`).
- Entry: `src/mock/main.tsx`, mounted only when Vite rewrites `index.html`.
- Scenario switcher is mock chrome, not product UI.

## Invariants

- Do not import this folder from `src/main.tsx` or `src/lib/tauri/client.ts`.
- Do not write files, touch `/Applications`, or call Audible.
- Do not change real `tauriClient` command behavior.
- Do not add a token/primitive catalog. Foundation + ProductionRoot are the
  visual references.

## Proof

- `bun run test -- src/mock scripts/ui-mock-entry-gate.test.ts`
- `bun run test -- src/lib/tauri-public-api.contract.test.ts`
- Browser: `bun run ui:mock` then switch scenarios and click import/encode/status.
