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
  truth and expose public-strip helpers for App Settings to call.
- Runtime settings controls consume backend capability facts for selectable
  accept/reject rules; do not add frontend-owned encoder/concurrency option
  matrices when a Rust owner validates the setting.
- Keep business logic in `.ts`/`.svelte.ts` modules and keep Svelte components focused on rendering and interaction.
- Use `src/types/*` for boundary-safe frontend typing when crossing TS↔Rust surfaces.
- Keep `src/styles.css` limited to the global base layer: Tailwind import, shared `@theme` tokens, shared shell/layout/dialog/form primitives, and truly app-wide rules. Component-specific visual styling should live in Svelte markup via utilities or in narrowly scoped component styles.
- Route UI proof-of-done through targeted tests for deterministic behavior and external browser-agent or human review when visual/UX judgment is the actual acceptance surface.
- Audiobook Boss is desktop-only, so alternate viewport review is out of scope unless a task explicitly asks for it.
- When touching metadata save/load behavior, open `src/lib/tauri/AGENTS.md` first.
- When touching Effect workflow owners or the AppEffect kernel, open
  `src/lib/effect/AGENTS.md` first.
- Treat function/file size as readability triggers: extract helpers when component scripts become hard to scan or test.

## Hard Invariants

- Runtime modules do not call command/event invokers directly from `src/lib/generated/tauri.ts`.
- Do not hand-edit `src/lib/generated/tauri.ts`; regenerate or sync bindings through the standard scripts.
- Keep runtime entry surfaces declarative: avoid new imperative DOM orchestration in `src/App.svelte`, `src/main.ts`, and `src/lib/**`.
- UI-affecting changes are not “done” from static inspection alone; they must leave targeted test coverage or explicit visual/UX review evidence for the user-facing outcome.
- Follow fallback policy from root `AGENTS.md` for any compatibility fallback introduced in frontend flows.
- Keep TypeScript boundaries type-safe; avoid introducing new `any` escape paths in runtime IPC/state flows.

### Frontend Shape Triggers

- Prefer colocated, testable logic modules over large monolithic component scripts.
- If a component or logic module approaches `~350` LOC, run a cohesion split check before extending it.
- If a function becomes branch-heavy or exceeds comfortable scan size, split into named helpers by user-facing behavior.
- For linting upgrades, prioritize type-aware `typescript-eslint` rules that catch unsafe `any` propagation.

## Canary Trigger

- Trigger Canary when frontend behavior depends on hidden coupling between UI modules and the Tauri boundary.
- Report the coupling, the working assumption used, and a minimal doc update proposal.
- Continue delivery unless the ambiguity threatens contract correctness or user data behavior.

## Done Criteria

- Tauri runtime calls are centralized through `tauriClient`.
- Metadata and IPC changes align with `src/lib/tauri/AGENTS.md` invariants.
- Global theme/base and shared shell/layout/dialog/form primitives resolve through `src/styles.css`; component-owned styling changes stay with the owning Svelte surface.
- UI-facing changes have targeted tests and, when needed, explicit visual/UX review evidence for the touched surface.
- Validation matches scope (`scripts/proof.sh standard` for non-doc code changes).
