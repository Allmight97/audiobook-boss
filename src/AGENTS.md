# Frontend Directives

## Scope

- Applies to frontend runtime and UI work under `src/`.
- IPC adapters live under `src/lib/tauri/AGENTS.md`; canonical metadata
  intent validation and normalization live in the Rust Metadata Outcome
  boundary.

## Preferred Path

- Route every runtime Tauri command/event through `src/lib/tauri/client.ts` (`tauriClient`).
- Route durable preference hydration and persistence through
  `src/ui/appSettings`; existing control owners keep their runtime request
  truth and expose documented public helpers for App Settings to call.
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
- Keep `src/styles.css` limited to the global base layer: Tailwind import, shared `@theme` tokens, shared shell/layout/dialog/form primitives, and truly app-wide rules. Component-specific visual styling should live in owner CSS.
- Route UI done evidence through targeted tests for deterministic behavior and external browser-agent or human review when visual/UX judgment is the actual acceptance surface.
- Audiobook Boss is desktop-only, so alternate viewport review is out of scope unless a task explicitly asks for it.
- When touching metadata save/load IPC, patch intent, or boundary
  normalization, open `src/lib/tauri/AGENTS.md` first; when touching
  metadata-save lifecycle display, also open `src/app/workOperations/AGENTS.md`.
- When touching Effect workflow owners or the AppEffect kernel, open
  `src/lib/effect/AGENTS.md` first.
- Treat hard-to-scan or hard-to-test component scripts as a signal to extract helpers at user-facing behavior boundaries.

## Design-System Primitives

- Design tokens are named values in `@theme`/`:root`; primitives are reusable
  styling behaviors/classes in `src/styles.css`; design-system work means
  changing that shared layer intentionally.
- Promote component styling to a primitive only when the deletion test passes:
  deleting the class would make the same layout or visual rules reappear across
  at least two UI owners. Keep one-owner styling local.
- For broad UI redesign work, decide or update tokens and the primitive kit
  first, then rebuild owner islands on top of that kit. Do not deepen around
  today's duplicated styles before the redesign unless the primitive already
  has stable multi-owner use.
- The design lab (`lab.html` + `src/lab/`, dev-only: served by the Vite dev
  server, not part of the app build) renders every token and primitive with a
  density switcher. When adding or changing a token or `src/styles.css`
  primitive, add/update its lab rendering in the same change — the lab is the
  visual-review surface for design-system work (screenshot it for evidence).
- Density is a user preference: read `--density-*` tokens instead of
  hardcoding row heights/padding; `[data-density='compact']` on `<html>`
  flips them.

## Hard Invariants

- Runtime modules do not call command/event invokers directly from `src/lib/generated/tauri.ts`.
- Do not hand-edit `src/lib/generated/tauri.ts`; regenerate or sync bindings through the standard scripts.
- Keep runtime entry surfaces declarative: avoid new imperative DOM orchestration in `src/ui/App.tsx`, `src/main.ts`, and `src/lib/**`.
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
- Global theme/base and shared shell/layout/dialog/form primitives resolve through `src/styles.css`; component-owned styling changes stay with the owning Solid view.
- UI-facing changes have targeted tests and, when needed, explicit visual/UX review evidence for the touched surface.
- Validation matches scope with direct commands from `README.md` and `scripts/AGENTS.md`.
