# Frontend Application Owners

## Scope

`src/app/<owner>` modules hold frontend session truth and effectful workflow
coordination. Solid views under `src/ui/<owner>` render these owners and
dispatch intent; they do not keep parallel business state.

## Owner Interface

- Treat each owner as a deep module. `index.ts` is its exact Public API Strip;
  import from the owner root and do not reach into private state, workflow,
  cache, binder, or helper files. App Runtime composition and cross-owner
  production modules use those owner roots.
- Prefer a small `view()` / accessor surface plus semantic intents such as
  import, select, stage, review, submit, cancel, or persist. Do not expose raw
  setters, refresh/poke functions, or one accessor per implementation field.
- Cross-owner coordination uses another owner's Public API Strip or an Effect
  workflow owned by the full outcome. Inject owner dependencies when the App
  Runtime composes them; do not look up the last-created owner from a module slot.
- Cross-owner integration tests exercise public owner intents. Owner-internal
  domain/workflow tests may import private modules to prove behavior at its
  cheapest stable boundary; Effect harness guidance lives in
  `src/lib/effect/AGENTS.md`. Do not widen the public strip solely for a test.

## State And Lifetime

- `createAppRuntime()` creates one instance of every session owner inside one
  Solid root. Mutable session truth belongs to that owner instance and is
  disposed with it.
- Keep screen-local disclosure, focus, and transient input in the Solid view;
  keep durable preferences in App Settings; keep accepted background operation
  truth in WorkRuntime.
- Derived views are computed from owner truth, not mirrored into another
  writable store. Capability and validation facts stay with their Rust owner.
- Disposal invalidates async generations and subscriptions before late results
  can publish. A remount or a second live runtime must not see or reset a
  sibling runtime's state.

## Workflow And Failure Shape

- Input owns awaitable selection and removal transitions; Metadata's draft gate
  must accept before Input changes. Pending targeted intents retain file
  identity across reordering and expire when the session is replaced/reset.
  Input cancels the draft gate on a newer transition, replacement/reset, or the
  requesting workflow's abort. An aborted gate cannot stage drafts, clear dirty
  state, or publish validation errors.
  Dependent workflows proceed only after selection and hydration succeed.
- Choose AppEffect when its typed failure, dependency composition, or scoped
  work reduces coordination; direct capability workflows may use plain async.
  Read `src/lib/effect/AGENTS.md` before changing that shape.
- Keep Effect programs and live layers private to the workflow owner. Public
  owner entrypoints return Promise or synchronous domain outcomes.
- Runtime calls route through `tauriClient`. Normalize user-facing errors and
  cancellation through `src/lib/tauri/appError.ts`; preserve typed provider
  diagnostics and backend terminal verdicts.
- Automatic persistence is an App Settings intent with observable durability
  state. Its acceptance, retry, and reset contract lives in
  `src/app/appSettings/AGENTS.md`.
- Publish observable state through the owner view and existing runtime/log
  surfaces. Do not add a shadow event bus or log-derived state machine.

## Done

- The owner has one session truth, one public interface, and one disposal path.
- Cross-owner reads use public strips; views render and dispatch only.
- Focused owner tests prove semantic outcomes and lifetime races through the
  public interface. Add App Runtime two-instance proof when isolation changes.
- Update a nested owner `AGENTS.md` only for non-obvious local invariants or
  public-surface changes; keep mutable execution state out of instructions.
