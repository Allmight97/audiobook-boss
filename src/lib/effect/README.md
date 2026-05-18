# AppEffect Workflow Owner Shape

Use `AppEffect` for frontend workflow owners that coordinate async work,
dependencies, typed failures, cleanup, or multi-service orchestration.

Keep Effect private to workflow owners:

- Public UI/runtime entrypoints expose Promise-returning functions.
- Workflow owners expose a local service tag, live layer, typed errors, program,
  and Promise bridge.
- Dependencies are injected through service objects so tests can provide fake
  layers without Svelte rendering or live Tauri.
- State and event outputs stay explicit in the owner contract, usually through a
  runtime context object supplied by the owning grey-box module.
- Public-strip impact is stated by the wrapper: if callers do not need new
  symbols or changed return types, keep the existing public strip stable.
- Scenario tests should prove visible workflow outcomes, cleanup/lifetime
  handoff, cancellation behavior, terminal results, and typed failure handling.

EB2 harness proof:

- `MetadataSaveWorkflow` uses the same private owner/service/layer pattern for
  metadata save orchestration while keeping Rust as metadata write truth.
- Its direct fake-layer tests cover no-op terminal paths, validation stops,
  success cleanup, partial failure retention, thrown infrastructure failures,
  and typed error mapping without rendering the full Svelte app.
- Focused command: `bun run test -- src/ui/core/__tests__/metadataSaveWorkflow.test.ts`

Future workflow ingress:

- New frontend work that coordinates multiple real boundaries should start with
  an explicit workflow owner, service contract, live layer, and fake-layer
  tests.
- Plain local UI state, pure transforms, and single-boundary event handlers can
  stay vanilla TypeScript when Effect would not clarify ownership or testing.

EB3 harness proof:

- `MetadataLookupWorkflow` owns metadata lookup search, selected-file queue
  progression, result application, and lookup-result cover-art replacement.
- Manual cover-art file, URL, drop, and clear loading remains vanilla in
  `src/ui/coverArt.ts` until that flow proves it needs a workflow owner.
- Focused command: `bun run test -- src/ui/metadataLookup/__tests__/metadataLookupWorkflow.test.ts`
