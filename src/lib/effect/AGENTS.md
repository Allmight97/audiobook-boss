# AppEffect Kernel Guidance

## Scope

- Applies to ABB's local Effect kernel and conventions under `src/lib/effect/`.
- Keep Effect as workflow-owner infrastructure, not a general UI dependency.

## Workflow Owner Shape

Use `AppEffect` for frontend workflow owners that coordinate async work,
dependencies, typed failures, cleanup, cancellation/lifetime handoff, or
multi-service orchestration.

Keep Effect workflow APIs private to workflow owners:

- Only `src/lib/effect/appEffect.ts` imports the `effect` package root. Owners
  and tests import `Effect`, `Context`, `Data`, and `Layer` from that file.
  Do not import `effect/unstable/reactivity` or `@effect/atom-solid`. Proof:
  `bun run test -- scripts/frontend-toolchain-layout.test.ts`.
- Public UI/runtime entrypoints expose Promise-returning functions or existing
  synchronous wrappers where callers already rely on them.
- When a workflow needs an Effect service layer, keep its service interface,
  tag, live layer, typed errors, and Promise bridge private to that owner.
  Co-locate the live layer with `satisfies`. Processing keeps live deps in
  `src/app/processing/workflow.deps.ts` (dynamic-imported) to preserve the
  cycle break.
- Inject dependencies so tests can supply capabilities or fake layers without
  Solid rendering or live Tauri.
- State and event outputs stay explicit in the owner contract.
- Public API Strip impact is stated by the wrapper: if callers do not need new
  symbols or changed return types, keep the existing public API stable.
- Scenario tests prove visible workflow outcomes, cleanup/lifetime handoff,
  cancellation behavior where relevant, terminal results, and typed failure
  handling.

## Workflow Harness Helpers

Use `makeWorkflowKit` when an owner needs a service tag, live-layer factory,
and tagged failure wrappers:

```ts
const kit = makeWorkflowKit(
	'Owner/WorkflowServices',
	'OwnerWorkflowFailed',
)<OwnerWorkflowServices>();
```

- Re-export `kit.Failed` as the owner's `*WorkflowFailed` (const + type alias)
  so call sites and tests keep the owner-named class.
- Use `kit.trySync` / `kit.tryPromise` for service calls; do not reintroduce
  local `Effect.tryPromise` / `Effect.try` blocks or hand-copied
  `Data.TaggedError` trios.
- The failure tag stays a per-owner string literal, so `Effect.catchTag`
  discriminates owners (pinned by `appEffect.test.ts`).
- Escape hatch: an owner with genuinely unique failure mapping keeps its own
  factory built on `kit.Failed` (for example `ProcessingWorkflow` normalizes
  `AppError` into the message and forks a hand-written
  `ProcessingWorkflowCancelled`).
- `workflowTryPromise` / `workflowTrySync` remain exported for the escape-hatch
  path; kit wrappers are the default.
- A program whose existing capability already supplies its dependencies can
  use the kernel directly, as in `src/app/inputSession/importWorkflow.ts`.
  Introduce a service layer when it clarifies a real dependency or failure
  boundary.

## Fake-Layer Harness Shape

For service-layer workflows, run the Effect program directly with fake services:

- Build a small `makeHarness` helper in the owner test file.
- Provide dependencies through the owner service layer, not through Solid
  rendering or live Tauri.
- Prefer assertions on public workflow outcomes: visible status, state writes,
  cleanup, terminal results, user-safe errors, and typed workflow errors.
- Keep fake services close to the owner test unless a second owner needs the
  same harness shape.
- Use live layers only from UI/runtime entrypoints; tests for owners should
  import the service layer helper and call the program or Promise bridge.

## Finding Workflow Owners

Find workflow files with
`rg --files src/app -g '*workflow*.ts' -g '*Workflow*.ts'`.
Tests may be siblings or under the owner's `__tests__/`; inspect the selected
owner for its focused proof. The Processing live-layer exception is described
above.

## Future Workflow Ingress

- New multi-boundary coordination belongs to an explicit workflow owner with
  injected dependencies and proof through its observable outcomes. Choose
  direct capability injection or a service layer using the criteria above.
- Plain local UI state, pure transforms, and single-boundary event handlers can
  stay vanilla TypeScript when Effect would not clarify ownership or testing.
