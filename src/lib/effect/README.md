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
