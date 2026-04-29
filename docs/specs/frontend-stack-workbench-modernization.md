# Frontend Stack and Workbench Modernization

## Purpose

Keep ABB's frontend stack intentional, current, and easy for agents to maintain without turning ABB into a different product.

ABB is not a library manager.
ABB is not a job-history app.
ABB is a desktop workflow engine for making audiobook files final, tagged, compatible, and library-ready.

## Stack Decision

The chosen stack is:

```text
Svelte 5       = intent/rendering surface
Vite           = build/dev system
Tailwind 4     = styling vocabulary
Bun            = JS package/script runner
Vitest         = frontend behavior proof
Tauri IPC      = truth boundary
Rust           = durable processing/artifact truth
```

Do not migrate to React, Next, SvelteKit, or another route/data framework unless ABB's product shape changes from an active desktop workbench into a route-addressed web/app framework problem.

## Truth Boundary

Use this vocabulary for frontend decisions:

```text
Rust produces durable truth.
IPC carries truth without drift.
UI renders truth and makes user intent easy to change.
```

Frontend work should preserve `src/lib/tauri/client.ts` as the runtime IPC boundary and avoid teaching UI panels to guess backend state outside the generated contract.

## Work Lanes

### 1. Tailwind Vite Plugin

Status: in progress in the first implementation PR.

Replace Tailwind's PostCSS adapter with the official Vite plugin:

```text
@tailwindcss/postcss + postcss.config.js
->
@tailwindcss/vite in vite.config.ts
```

This is a build-pipeline cleanup only. It must not change UI behavior or flow.

Acceptance:

- `postcss.config.js` is gone.
- `@tailwindcss/postcss` is not a direct dependency.
- `postcss` is not a direct dependency unless a non-Tailwind owning use appears.
- `autoprefixer` is not a direct dependency unless a non-Tailwind owning use appears.
- `@tailwindcss/vite` is installed and configured in `vite.config.ts`.
- `src/styles.css` remains the global Tailwind/theme entrypoint.
- `bun ci`, supply-chain checks, and `scripts/checks.sh standard` pass.

### 2. Declarative Islands and Panels

Status: future PRs by owned flow.

Continue migrating old frontend shape toward declarative Svelte islands/panels and `.svelte.ts` state where that removes imperative DOM orchestration or hidden coupling.

Do not mix broad island/panel migration with build-pipeline dependency cleanup. Keep future PRs organized by owned user flow, such as:

- file import/order
- metadata intent
- output plan
- processing/status
- collision/review dialogs

Acceptance for each future lane:

- Components render intent/truth; logic lives in focused `.ts` or `.svelte.ts` modules.
- Runtime Tauri calls still route through `tauriClient`.
- Targeted Vitest coverage proves the touched behavior.
- UI-facing changes include explicit visual/UX review evidence when behavior cannot be proven statically.

### 3. Workbench Information Architecture

Status: deferred.

The current hypothesis is:

```text
Workbench, not wizard.
Phases, not pages.
Guidance, not gates.
Review points, not mandatory ceremony.
```

This lane needs hands-on UX feel-testing before implementation. Do not treat this spec as approval for a UI redesign.

The deferred Obsidian note is:

```text
/Users/jstar/Library/Mobile Documents/iCloud~md~obsidian/Documents/Main Vault/00_Inbox/2026-04-29 ABB workbench information architecture option.md
```

## Guardrails

- Optimize for one active batch, not durable library/job management.
- Avoid half-migrations: if a legacy frontend build adapter is replaced, remove the old adapter completely.
- Keep product-flow language separate from route-framework language unless routes become a real product need.
- Validation for code/config/build changes is `scripts/checks.sh standard`.
