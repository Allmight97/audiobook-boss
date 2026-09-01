# AppEffect Kernel Guidance

## Scope

- Applies to ABB's local Effect kernel and conventions under `src/lib/effect/`.
- Keep Effect as workflow-owner infrastructure, not a general UI dependency.

## Workflow Owner Shape

Use `AppEffect` for frontend workflow owners that coordinate typed failures,
Layer-injected dependencies, cleanup/lifetime handoff, or multi-service
orchestration. Do not wrap generation-gated async or Result-mapping
single-boundary work in Effect: Input import (`runImportIntent`) and Remote
Source (`runRemoteSourceWorkflow`) stay injectable vanilla TypeScript.

Keep Effect workflow APIs private to workflow owners:

- Only `src/lib/effect/appEffect.ts` imports the `effect` package root. Owners
  and tests import `Effect`, `Context`, `Data`, and `Layer` from that file.
  Do not import `effect/unstable/reactivity` or `@effect/atom-solid`. Proof:
  `bun run test -- scripts/frontend-toolchain-layout.test.ts`.
- Public UI/runtime entrypoints expose Promise-returning functions or existing
  synchronous wrappers where callers already rely on them.
- Workflow owners expose a local service interface, service tag, live layer
  (co-located in the workflow file with `satisfies`), typed errors, program,
  and Promise bridge. Processing keeps live deps in
  `src/app/processing/workflow.deps.ts` (dynamic-imported) to preserve the
  cycle break.
- Dependencies are injected through service objects so tests can provide fake
  layers without Solid rendering or live Tauri.
- State and event outputs stay explicit in the owner contract.
- Public API Strip impact is stated by the wrapper: if callers do not need new
  symbols or changed return types, keep the existing public API stable.
- Scenario tests prove visible workflow outcomes, cleanup/lifetime handoff,
  cancellation behavior where relevant, terminal results, and typed failure
  handling.

## Workflow Harness Helpers

Workflow owners get their service tag, live-layer factory, tagged failure
class, failure factory, and try-wrappers from one kernel kit (#389):

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
  discriminates owners exactly like hand-written classes (pinned by the kit
  spike tests in `appEffect.test.ts`).
- Escape hatch: an owner with genuinely unique failure mapping keeps its own
  factory built on `kit.Failed` (for example `ProcessingWorkflow` normalizes
  `AppError` into the message and forks a hand-written
  `ProcessingWorkflowCancelled`).
- `workflowTryPromise` / `workflowTrySync` remain exported for the escape-hatch
  path; kit wrappers are the default.

## Fake-Layer Harness Shape

Workflow tests should run the Effect program directly with fake services:

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

Effect workflow owners live in `*Workflow*.ts` files that import the AppEffect
kernel; find the current set by searching the tree (e.g.
`rg -l "makeWorkflowKit" src/app`) rather than from a hand-maintained list.
Each Effect owner co-locates its service interface, service tag, live layer
(`satisfies`), program, and Promise bridge, and is exercised by a focused
Vitest file next to it (`bun run test -- <owner test file>`). Files named
`*Workflow*.ts` that do not import the kernel (Input import, Remote Source)
are vanilla injectable async, not AppEffect owners.

Public API Strip impact for these owners is intentionally narrow: callers keep
existing UI/runtime Promise or synchronous wrapper shapes unless a milestone
explicitly accepts a public API change.

## Future Workflow Ingress

- New frontend work that coordinates multiple real boundaries should start with
  an explicit workflow owner, co-located service contract + live layer, and
  fake-layer tests.
- Plain local UI state, pure transforms, and single-boundary event handlers can
  stay vanilla TypeScript when Effect would not clarify ownership or testing.
