# Work Center

## Public API Strip

- `WorkCenterIsland`
- `initializeWorkCenter`
- `readWorkActivityByInputId`
- `workCenterState`

## Ownership

- Own the frontend read model for WorkRuntime operations, operation-level cancel
  actions, and source actions.
- Consume Tauri only through `src/lib/tauri/client.ts`.
- Do not own processing submission preparation, metadata staging, output plan
  review, provider auth, or remote materializer details.

## Lifecycle Truth

- Work Center renders **only** backend-authored WorkRuntime snapshot events
  (`work-operation-snapshot`, `work-operation-list-snapshot`). These snapshots
  carry in-flight progress detail, child-job state, cancellability, and
  terminal summaries.
- Work Center does **not** subscribe to `processing-progress` events, import
  `ProcessingProgressEvent`, or apply client-authored progress overlays for accepted
  background operations. The `OperationSnapshot` is the sole progress source.
- Terminal operation status is backend-canonical through
  `abb_processing_core::classify_run_terminal`. Work Center does not
  recalculate success, mixed, failed, skipped, or cancelled outcomes.

## Guardrails

- Keep multi-operation state in `model.ts`/`state.svelte.ts`; the Svelte
  component should render and dispatch actions only.
- Do not push singleton queue logic back into StatusPanel.
- Never reintroduce `processing-progress` overlay consumption for background
  operations.
