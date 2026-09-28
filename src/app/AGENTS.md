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
- Input owns output titles and each title's ordered source files. The visible
  `fileList` entries are stable metadata anchors; `titleSourcesByIdentity`
  contains composition, and public `sourceFiles`/`selectedSourceFiles` include
  hidden sources. A source reorder never changes the metadata anchor or title
  identity. Grouping uses list order and the first selected title's metadata;
  conflicting audio choices require an explicit title-level choice.
  Grouping/separation use Metadata's draft gate. Separation keeps the title's
  edits on its metadata anchor and restores the other sources' drafts.
- Filename sort rewrites the processing order by natural numeric basename;
  a manual reorder clears the sort claim, and selection follows file identity
  through both.
- Metadata Session alone stages metadata intent. Its cache derives what the
  form shows from each file's known tags plus pending intent (`cache.ts`), so
  callers never merge or compare intent themselves. The cache owns source-read
  acceptance: a read begun before an acknowledged save cannot replace saved
  tags, and removal/reset invalidates outstanding reads. Draft preparation,
  hydration, and cover discovery all read through that owner. Saved values
  without a source read remain partial knowledge, not a complete baseline;
  unknown source values cannot justify dropping an explicit Blank as unchanged.
- Metadata drafts for hidden sources survive grouping. Saving a grouped draft
  stages it for the output; it never writes that draft into a constituent
  source. Lookup targets the visible title once. Remote retention and summaries
  use all source identities, not only visible metadata anchors.
- Input owns each title's complete audio request by stable identity. Runtime
  initializes stored preferences before import; Input snapshots defaults as new
  titles enter the list. Reordering preserves choices; removal/reset clears them.
  Source recommendations stay Audio-owned; views do not infer pass-through.
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
- Metadata edit-retention tests collect the first processing request, make a
  later edit, and collect again; assert earlier text/cover intent survives.
  Control read/save completion order when testing freshness. Actual output-tag
  proof belongs to the Rust processing workflow (see its owner guidance).
- Update a nested owner `AGENTS.md` only for non-obvious local invariants or
  public-surface changes; keep mutable execution state out of instructions.
