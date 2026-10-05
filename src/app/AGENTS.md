# Frontend Application Owners

## Scope

`src/app/<owner>` modules give Solid views one interface per product area.
Views under `src/ui/<owner>` render these owners and dispatch intent; they do
not keep parallel business state.

## Engine Adapters

- Input, metadata, lookup, settings, encoding, output, preview, remote choices,
  and Work Center progress render Rust-owned facts. Adapters word typed statuses
  and send intents or the owned read/cancel API; product rules go in the engine.
- Keep frontend lifetime and presentation resources here: dialog disclosure,
  transient typing echo, visible filter/sort, and authorization browser
  opening. They never determine accepted file work or terminal truth.

## Engine Link

- `engineLink` is the one connection to the engine. It keeps the newest copy
  of each snapshot part (titles, selection, metadata, lookup, audio, output,
  remote, remote library, settings) by
  revision, and sends intents. Adapters read the link; nothing else holds a
  copy of engine state.
- `send` resolves with the intent's outcome after its work finishes; `post`
  is for intents whose outcome nobody awaits. Both are numbered in the order
  called, and the host runs them in that order.
- A part that arrives unchanged keeps object identity for its files and lookup
  results, so Solid rows are not rebuilt and a click does not land on a
  replaced element.
- Adapters show typed text (form fields, lookup queries, the naming template)
  immediately and drop it when the engine's reply for that keystroke arrives. Form typing shows only on the
  form it was typed into (the metadata part's `binding`). This local echo is
  display only; it never decides what is saved.

## Owner Interface

- Treat each owner as a deep module. `index.ts` is its exact Public API Strip;
  import from the owner root. App Runtime composition and cross-owner
  production modules use those owner roots.
- Prefer a small `view()` / accessor surface plus semantic intents. Do not
  expose raw setters, refresh/poke functions, or one accessor per field.
- Cross-owner coordination uses another owner's Public API Strip. Inject owner
  dependencies when the App Runtime composes them.
- Cross-owner integration tests exercise public owner intents. Owner-internal
  tests may import private modules to prove behavior at its cheapest stable
  boundary. Do not widen the public strip solely for a test.

## State And Lifetime

- `createAppRuntime()` creates the engine link and one instance of every owner
  inside one Solid root, and disposes them together. A late engine reply after
  disposal changes nothing visible.
- Keep screen-local disclosure, focus, and transient input in the Solid view;
  keep accepted background operation truth in WorkRuntime.
- Derived views are computed from owner truth, not mirrored into another
  writable store. Capability and validation facts stay with their Rust owner.

## Workflow And Failure Shape

- Owner workflows are plain async. Public owner entrypoints return Promise or
  synchronous domain outcomes.
- Runtime calls route through `tauriClient`. Normalize user-facing errors and
  cancellation through `src/lib/tauri/appError.ts`; preserve typed provider
  diagnostics and backend terminal verdicts.
- Publish observable state through the owner view and existing runtime/log
  surfaces. Do not add a shadow event bus or log-derived state machine.

## Done

- The owner has one source of truth, one public interface, and one disposal
  path. Cross-owner reads use public strips; views render and dispatch only.
- Adapter and UI tests run against `src/test/fixtures/fakeEngine.ts`. It
  records every intent, applies plain list mechanics (import append,
  selection, removal, output echo), renders the form from seeded tags and
  typed values, and answers settings intents with a small write model. It
  copies no engine decision: grouping, ordering, Save, lookup, cover loads,
  audio edits, and remote product rules are recorded only. A test that needs the
  engine's answer seeds it with `change`, `respond`, `answerSubmission`, or a
  `seed*` method; never add a product rule there.
- Add App Runtime two-instance proof when isolation changes.
- Update a nested owner `AGENTS.md` only for non-obvious local invariants or
  public-surface changes; keep mutable execution state out of instructions.
