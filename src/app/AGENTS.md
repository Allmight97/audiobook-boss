# Frontend Application Owners

## Scope

`src/app/<owner>` modules give Solid views one interface per product area.
Views under `src/ui/<owner>` render these owners and dispatch intent; they do
not keep parallel business state.

## Two Kinds Of Owner

- **Engine adapters**: `inputSession`, `metadataSession`, `metadataLookup`,
  `appSettings`, `encoding`, `outputPlan`. The engine owns their truth and rules
  (`crates/abb-engine/src/session/AGENTS.md`,
  `crates/abb-engine/src/app_settings/AGENTS.md`). The adapter turns engine
  snapshots into view state, words typed statuses and notices, and sends
  intents through `engineLink`. A new rule for titles, selection, metadata
  edits, lookup, Save, audio choices, output naming, or settings goes in the
  engine, not here.
- **Frontend owners**: `processing`, `workOperations`, `remoteSource`, and the
  collision review in `outputPlan`. They still hold their own workflow until
  it moves into the engine. New product rules go in the engine even here.

## Engine Link

- `engineLink` is the one connection to the engine. It keeps the newest copy
  of each snapshot part (titles, selection, metadata, lookup, audio, output,
  settings) by
  revision, and sends intents. Adapters read the link; nothing else holds a
  copy of engine state.
- `send` resolves with the intent's outcome after its work finishes; `post`
  is for intents whose outcome nobody awaits. Both are numbered in the order
  called, and the host runs them in that order.
- A part that arrives unchanged keeps object identity for its files and lookup
  results, so Solid rows are not rebuilt and a click does not land on a
  replaced element.
- Typed text (form fields, lookup queries, the naming template) shows immediately and drops when
  the engine's reply for that keystroke arrives. Form typing shows only on the
  form it was typed into (the metadata part's `binding`). This local echo is
  display only; it never decides what is saved.

## Owner Interface

- Treat each owner as a deep module. `index.ts` is its exact Public API Strip;
  import from the owner root. App Runtime composition and cross-owner
  production modules use those owner roots.
- Prefer a small `view()` / accessor surface plus semantic intents. Do not
  expose raw setters, refresh/poke functions, or one accessor per field.
- Cross-owner coordination uses another owner's Public API Strip or an Effect
  workflow owned by the full outcome. Inject owner dependencies when the App
  Runtime composes them.
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

## Temporary Until Processing Moves Into The Engine

These bridge engine-owned session state to the frontend owners that still
build processing requests. Remove them with that move. The engine intents
they use are listed in `crates/abb-engine/src/session/AGENTS.md`.

- Processing builds its payload from each title's `request` in the audio part
  and the output part's `naming`.
- Processing locks the list during a preview through Input's
  `setOrderLocked`, and reads pending edits for its payload through Metadata
  Session, which asks the engine (`metadataIntents`).
- Input exports `chapterPlansForProcessing`, which refuses an unconfirmed CUE
  and CUE chapters on a merged title before submit.

## Workflow And Failure Shape

- Choose AppEffect when its typed failure, dependency composition, or scoped
  work reduces coordination; direct capability workflows may use plain async.
  Read `src/lib/effect/AGENTS.md` before changing that shape.
- Keep Effect programs and live layers private to the workflow owner. Public
  owner entrypoints return Promise or synchronous domain outcomes.
- Runtime calls route through `tauriClient`. Normalize user-facing errors and
  cancellation through `src/lib/tauri/appError.ts`; preserve typed provider
  diagnostics and backend terminal verdicts.
- Publish observable state through the owner view and existing runtime/log
  surfaces. Do not add a shadow event bus or log-derived state machine.

## Done

- The owner has one source of truth, one public interface, and one disposal
  path. Cross-owner reads use public strips; views render and dispatch only.
- Adapter and UI tests run against `src/test/fixtures/fakeEngine.ts`. The
  fake mimics enough engine behavior (selection, import, grouping, staging,
  lookup, Save) for views to render; those copies prove nothing about the
  engine. Engine rules are proved in Rust. Prefer seeding a snapshot through
  the fake's `change` over adding behavior to it, and never add a new product
  rule there.
- Add App Runtime two-instance proof when isolation changes.
- Update a nested owner `AGENTS.md` only for non-obvious local invariants or
  public-surface changes; keep mutable execution state out of instructions.
