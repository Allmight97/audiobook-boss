# Frontend Directives

## Scope

- Applies to frontend runtime and UI work under `src/`.
- Application owner interfaces, state lifetime, and workflow shape live in
  `src/app/AGENTS.md`; read it before changing frontend session truth.
- IPC adapters live under `src/lib/tauri/AGENTS.md`; canonical metadata
  intent validation and normalization live in the Rust Metadata Outcome
  boundary.

## Preferred Path

- Route every runtime Tauri command/event through `src/lib/tauri/client.ts` (`tauriClient`).
- Route durable preference hydration and automatic persistence through
  `src/app/appSettings`; owning controls expose semantic accepted values for
  Settings to persist. `src/ui/appSettings` is a current compatibility
  strip with hidden failure semantics—do not add callers.
- Runtime settings controls consume backend capability facts for selectable
  accept/reject rules; do not add frontend-owned encoder/concurrency option
  matrices when a Rust owner validates the setting.
- Remote acquisition lives under `src/app/remoteSource` with a Solid dialog in
  `src/ui/remoteSource`. It may coordinate account state, library display,
  selection, and acquired-file import handoff through Input's public strip.
  Provider secrets and raw provider payloads stay backend-only.
- Keep business logic in `.ts` modules and keep Solid views focused on
  rendering and interaction.
- Use `src/types/*` for boundary-safe frontend typing when crossing TS↔Rust surfaces.
- Keep `src/styles.css` as a thin bootstrap: it loads the UI foundation and
  owns app-shell layout only. Shared chrome and tokens live in
  `src/ui/foundation`. Owner-specific layout lives in that owner's CSS.
- Route UI done evidence through targeted tests for deterministic behavior and external browser-agent or human review when visual/UX judgment is the actual acceptance surface.
- Audiobook Boss is desktop-only, so alternate viewport review is out of scope unless a task explicitly asks for it.
- When touching metadata save/load IPC, patch intent, or boundary
  normalization, open `src/lib/tauri/AGENTS.md` first; when touching
  metadata-save lifecycle display, also open `src/app/workOperations/AGENTS.md`.
- When touching Effect workflow owners or the AppEffect kernel, open
  `src/lib/effect/AGENTS.md` first.
- New or migrated session truth belongs to an owner composed by App Runtime and
  consumed from Solid context. Presentation resources belong to their owner or
  view instance. Existing module-global/lifetime exceptions are not precedent;
  verify their current call sites before changing them and do not add another.
  Do not reintroduce Effect Atom or add a process-wide owner singleton. Proof:
  `bun run test -- scripts/frontend-toolchain-layout.test.ts`.
- Treat hard-to-scan or hard-to-test component scripts as a signal to extract helpers at user-facing behavior boundaries.

## UI Foundation

- Shared visual behavior crosses `src/ui/foundation`. Read
  `src/ui/foundation/AGENTS.md` before adding a primitive or token.
- Owner CSS consumes public semantic tokens. It does not import another
  owner's CSS or foundation internals.
- Browser mock-runtime (`bun run ui:mock`) mounts `ProductionRoot` against
  official Tauri mocks. It is not a token catalog and must stay out of
  `src/main.tsx`.

## Hard Invariants

- Runtime modules do not call command/event invokers directly from `src/lib/generated/tauri.ts`.
- Do not hand-edit `src/lib/generated/tauri.ts`; regenerate or sync bindings through the standard scripts.
- Keep runtime entry surfaces declarative: avoid new imperative DOM orchestration in `src/ui/App.tsx`, `src/main.tsx`, and `src/lib/**`.
- UI-affecting changes are not “done” from static inspection alone; they must leave targeted test coverage or explicit visual/UX review evidence for the user-facing outcome.
- Follow root Hard Invariants for boundary behavior; do not add hidden or caller-side substitute logic in frontend flows.
- Keep TypeScript boundaries type-safe; avoid introducing new `any` escape paths in runtime IPC/state flows.

### Frontend Shape Triggers

- Route shape questions through root Refactor Discipline.
- Split when scan cost, test cost, or ownership blur rises — at user-facing
  behavior boundaries.
- For linting upgrades, prioritize type-aware `typescript-eslint` rules that catch unsafe `any` propagation.

## Hidden Coupling Traps

- If frontend behavior depends on hidden coupling between UI modules and the Tauri boundary, name the coupling and working assumption used.
- Localize the behavior behind the owning boundary when it is part of the active task; otherwise report the smallest docs or test guard that would prevent recurrence.
- Continue only when the ambiguity does not threaten contract correctness or user data behavior.

## Done Criteria

- Tauri runtime calls are centralized through `tauriClient`.
- Metadata and IPC changes align with `src/lib/tauri/AGENTS.md` invariants.
- Shared chrome and tokens resolve through `src/ui/foundation`. App-shell
  layout stays in `src/styles.css`. Owner-specific styling stays with the
  owning Solid view.
- UI-facing changes have targeted tests and, when needed, explicit visual/UX review evidence for the touched surface.
- Validation matches scope with direct commands from `README.md` and `scripts/AGENTS.md`.
