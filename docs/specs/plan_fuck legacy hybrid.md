# Historical Execution Tracker: Zero-Legacy Svelte Cutover (Issue #236)

> Historical branch-era tracker only. This file is not current repo policy, not the canonical architecture source of truth, and not the verification source of truth. Use `docs/README.md`, `docs/specs/technical-reference.md`, and `docs/verification.md` for current guidance.

Snapshot date: 2026-02-20  
Branch baseline: `feat/zero-legacy-svelte-cutover`

## Scope posture

This tracker is implementation-first: what actually landed on this branch, what diverged from earlier assumptions, and what remains for strict full-Svelte runtime closure.

## Zero-Legacy Rule Clarification

Strict zero-legacy applies to both:

1. Runtime behavior (no legacy bridge/hybrid runtime contracts).
2. Test contracts (tests must not assert legacy runtime contracts).

Test fixtures may still use static HTML scaffolding (IDs/classes) when needed to mount components or simulate browser events. That is fixture plumbing, not runtime architecture.

## Status legend

- `completed`: implemented and validated on this branch.
- `partial`: meaningful migration landed; residual hybrid seams remain.
- `remaining`: not yet migrated.

## Phase tracker

| Phase | Status | Branch-coherent evidence | Remaining to close |
| --- | --- | --- | --- |
| 0. Branch + governance | completed | Working branch in place: `feat/zero-legacy-svelte-cutover`; bridge import guard active (`scripts/check-no-bridge-imports.sh`). | Keep active through merge.
| 1. Bridge retirement + typed boundary | completed | Bridge removed; typed Tauri boundary is canonical (`src/lib/tauri/client.ts`); generated bindings stay in lock-step via checks. | None.
| 2. Status/process/save flow migration | completed | Status panel moved to reactive view-state ownership (`src/ui/statusPanel/viewState.svelte.ts`, `src/ui/statusPanel/dom.ts`, `src/ui/statusPanel/StatusPanelIsland.svelte`); metadata save UX now uses transient status API (`src/App.svelte`, `src/ui/metadataSaveState.ts`). | None for this slice.
| 3. Output/encoder runtime contract cleanup | completed | `EncoderSettingsProvider` path removed; canonical processing selector is `readOutputConfigForProcessing()` (`src/ui/outputPanel/state.ts`); processing flow consumes typed output config (`src/ui/statusPanel/processing.ts`). | None for this slice.
| 4. File-list runtime hardening | completed | File list render/state remains island-driven; `fileList/dom.ts` is now state-only (removed imperative row builders and direct DOM button/text writes), combined size moved to reactive binding in `App.svelte`; order-lock tests now assert view-state behavior (`src/ui/__tests__/order-lock-lifecycle.test.ts`). | None for this slice.
| 5. Test + policy hardening | completed | Legacy test-contract guard added and wired (`scripts/check-no-legacy-test-contracts.sh`, `scripts/checks.sh`); fallback register updated for retired FB-014 entry (`docs/engineering/fallback-register.md`). | None for this slice.
| 6. Full runtime de-imperativization (strict final mile) | partial | Major hybrid hotspots retired (status + file-list control path + bridge layer). | Residual imperative runtime seams listed below.
| 7. Hardening + package gates | completed | `scripts/checks.sh standard` and `scripts/checks.sh package` pass on current branch head (including app bundle build). | Re-run once remaining final-mile seams are retired.

## Completed highlights (current head)

- Bridge retired from runtime (`src/lib/bridge.ts` absent).
- Typed IPC boundary and generated contracts are the only command/event path (`src/lib/tauri/client.ts`).
- Status panel runtime rendering moved to reactive state (no fallback renderer ownership in `statusPanel/dom.ts`).
- Metadata save in-progress is store-driven (`src/ui/metadataSaveState.ts`) and button state is island-bound (`src/ui/metadataForm/MetadataFormFieldsIsland.svelte`).
- File-list control state and combined-size display are reactive (no direct DOM writes in `src/ui/fileList/dom.ts`).
- Legacy test contract reintroduction is blocked in checks (`scripts/check-no-legacy-test-contracts.sh`).
- Guard scope ratcheted to include `src/ui/fileList/dom.ts` in runtime imperative scan (`scripts/check-no-imperative-dom-runtime.sh`).

## What did not go as initially assumed

1. Earlier tracker language over-stated completion in a few areas while some imperative runtime seams still existed.
2. File-list helper code still carried legacy direct DOM updates longer than expected; this was corrected in the current pass.
3. “Runtime-only” language caused confusion around tests. The policy is now explicit: legacy runtime contracts are disallowed in tests too; only fixture scaffolding is tolerated.

## Remaining work for strict 100% runtime zero-legacy

These are the last architectural seams still using imperative DOM orchestration patterns in runtime modules:

1. `src/ui/outputPanel/dom.ts`
   - Replace `getElementById`-driven preview/warning rendering with island/state bindings.
2. `src/ui/metadataForm.ts`
   - Move form read/write/dirty tracking from document queries to store-backed field state.
3. `src/ui/fileList/metadataPanel.ts`
   - Replace selected-file property rail DOM writes with reactive state + island binding.
4. `src/ui/encoderPanel/dom.ts` and related helpers
   - Retire remaining DOM cache/query wiring and centralize control state in reactive stores.
5. Runtime event bus cleanup
   - Reduce document-level custom event plumbing (`abb:*`) where store-driven flows can replace it.

## Final-mile acceptance criteria (strict closure)

1. Runtime feature modules no longer require direct DOM query/mutation orchestration for core UX state.
2. Legacy runtime contracts remain blocked in both runtime and tests by policy gates.
3. `scripts/checks.sh standard` and `scripts/checks.sh package` stay green after final-mile migrations.
