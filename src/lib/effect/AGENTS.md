# AppEffect Kernel Guidance

## Scope

- Applies to ABB's local Effect kernel and conventions under `src/lib/effect/`.
- Keep Effect as workflow-owner infrastructure, not a general UI dependency.

## Workflow Owner Shape

Use `AppEffect` for frontend workflow owners that coordinate async work,
dependencies, typed failures, cleanup, cancellation/lifetime handoff, or
multi-service orchestration.

Keep Effect private to workflow owners:

- Public UI/runtime entrypoints expose Promise-returning functions or existing
  synchronous wrappers where callers already rely on them.
- Workflow owners expose a local service tag, live layer, typed errors, program,
  and Promise bridge.
- Dependencies are injected through service objects so tests can provide fake
  layers without Svelte rendering or live Tauri.
- State and event outputs stay explicit in the owner contract.
- Public-strip impact is stated by the wrapper: if callers do not need new
  symbols or changed return types, keep the existing public strip stable.
- Scenario tests prove visible workflow outcomes, cleanup/lifetime handoff,
  cancellation behavior where relevant, terminal results, and typed failure
  handling.

## Future Workflow Ingress

- New frontend work that coordinates multiple real boundaries should start with
  an explicit workflow owner, service contract, live layer, and fake-layer
  tests.
- Plain local UI state, pure transforms, and single-boundary event handlers can
  stay vanilla TypeScript when Effect would not clarify ownership or testing.

## Current Proof Points

- `ProcessingWorkflow` validates the AppEffect kernel around processing
  orchestration.
- `MetadataSaveWorkflow` validates metadata save orchestration while keeping
  Rust as metadata write truth.
- `MetadataLookupWorkflow` validates metadata lookup queue progression and
  lookup-result cover-art replacement.
- Manual cover-art file, URL, drop, and clear loading remains vanilla in
  `src/ui/coverArt.ts` until that flow proves it needs a workflow owner.
