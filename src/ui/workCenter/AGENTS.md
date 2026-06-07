# Work Center

## Public API Strip

- `WorkCenterIsland`
- `initializeWorkCenter`
- `workCenterState`

## Ownership

- Own the frontend read model for WorkRuntime operations, operation-level cancel
  actions, and source actions.
- Consume Tauri only through `src/lib/tauri/client.ts`.
- Do not own processing submission preparation, metadata staging, output plan
  review, provider auth, or remote materializer details.

## Guardrails

- Keep multi-operation state in `model.ts`/`state.svelte.ts`; the Svelte
  component should render and dispatch actions only.
- Do not push singleton queue logic back into StatusPanel.
