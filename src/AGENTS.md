# Frontend Directives

## Scope

- Applies to frontend runtime and UI work under `src/`.
- This file keeps frontend behavior guidance concise.
- IPC and metadata intent source-of-truth lives in `src/lib/tauri/AGENTS.md`.

## Preferred Path

- Route every runtime Tauri command/event through `src/lib/tauri/client.ts` (`tauriClient`).
- Keep business logic in `.ts`/`.svelte.ts` modules and keep Svelte components focused on rendering and interaction.
- Use `src/types/*` for boundary-safe frontend typing when crossing TS↔Rust surfaces.
- Keep layout/token changes anchored to `src/styles.css` as the spacing and token source of truth.
- Route UI proof-of-done through the harness substrate (`bun run harness:verify --scenario <name>` or `--changed`) plus targeted tests for the touched surface.
- When touching metadata save/load behavior, open `src/lib/tauri/AGENTS.md` first.
- Treat function/file size as readability triggers: extract helpers when component scripts become hard to scan or test.

## Hard Invariants

- Runtime modules do not call command/event invokers directly from `src/lib/generated/tauri.ts`.
- Do not hand-edit `src/lib/generated/tauri.ts`; regenerate/sync bindings through the standard scripts/hooks.
- Preserve the migrated runtime posture: avoid new imperative DOM orchestration in `src/App.svelte`, `src/main.ts`, `src/harness-main.ts`, and `src/lib/**`.
- UI-affecting changes are not “done” from static inspection alone; they must leave targeted test coverage and harness verification evidence.
- If a UI change touches a surface with no matching harness scenario, add or extend the scenario instead of silently skipping proof.
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
- Token/spacing changes resolve through `src/styles.css` source-of-truth.
- UI-facing changes have targeted tests plus harness verification coverage for the touched surface.
- Validation matches scope (`scripts/checks.sh standard` for non-doc code changes).
