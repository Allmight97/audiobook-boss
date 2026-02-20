# Zero-Legacy Svelte Cutover - Execution Tracker (Issue #236)

Snapshot date: 2026-02-20  
Branch baseline: `feat/zero-legacy-svelte-cutover`  
Owner lane focus in this document: delivery tracking (completed / partial / remaining), not architecture ideation.

## Current posture

The cutover has already landed major contract work (bridge retirement + `tauriClient` boundary) and a Svelte app shell, but the runtime is still hybrid in feature modules (`src/ui/**`) with imperative DOM orchestration.  
Outcome framing: user-facing flows run on the new IPC boundary, but maintainability risk remains until legacy DOM orchestration is fully retired.

## Status Legend

- `completed`: Exit criteria reached on current branch
- `partial`: Meaningful progress landed, but exit criteria not yet met
- `remaining`: Not started or blocked behind prior phase work

## Phase Tracker

| Phase | Status | Branch-coherent evidence | Remaining work to close |
| --- | --- | --- | --- |
| 0. Branch + governance | completed | Branch exists (`feat/zero-legacy-svelte-cutover`); explicit no-bridge guard is active (`scripts/check-no-bridge-imports.sh`). | Keep policy enforcement active during merge prep. |
| 1. Testing / harness foundation | completed | Harness entry exists (`src/harness-main.ts`, `src/HarnessApp.svelte`); Vitest setup and frontend tests active (`src/test/setup.ts`, `src/ui/**/__tests__`). | Continue coverage as migration closes remaining legacy surfaces. |
| 2. App shell + state model | partial | `src/App.svelte` is root composition and `src/main.ts` is mount-only. | Move state ownership from module singletons/DOM flows to feature stores (`.svelte.ts`) across remaining runtime features. |
| 3. Wave 1 (encoder/output/job/cover) | partial | Svelte islands exist (`src/ui/*/*Island.svelte` for encoder/output/job/cover); `jobControls` runtime path is store/island-driven (`src/ui/jobControls.ts`, `src/ui/jobControls/state.svelte.ts`, `src/ui/jobControls/JobControlsIsland.svelte`). | Retire remaining imperative DOM orchestration in `src/ui/encoderPanel/**`, `src/ui/outputPanel/**`, and `src/ui/coverArt.ts`. |
| 4. Wave 2 (import/list/metadata/tag/lookup) | partial | Island components exist for key surfaces (`FileImportIsland.svelte`, `MetadataFormFieldsIsland.svelte`, `MetadataLookupIsland.svelte`, `TagPreviewIsland.svelte`). | Complete migration away from imperative DOM-heavy modules (`src/ui/fileImport.ts`, `src/ui/fileList/**`, `src/ui/metadataForm.ts`, `src/ui/metadataLookup.ts`, `src/ui/tagPreview.ts`). |
| 5. Processing/status/save flow | partial | Status panel island exists (`src/ui/statusPanel/StatusPanelIsland.svelte`) and queue/progress flow is typed through `tauriClient.listen`. | Finish removing legacy status DOM renderer/orchestrator layers (`src/ui/statusPanel/dom.ts`, `src/ui/statusPanel/events.ts`, `src/App.svelte` save orchestration coupling). |
| 6. Legacy retirement + enforcement | partial | `src/lib/bridge.ts` is retired; boundary is now `src/lib/tauri/client.ts`; no-bridge script wired in checks; no-imperative guard now includes migrated `jobControls` runtime paths. | Expand no-new imperative DOM policy to each additional feature path as modules are migrated. |
| 7. Hardening + merge | partial | `scripts/checks.sh standard` passes on current branch head after jobControls migration + test updates. | Run `scripts/checks.sh package` and merge smoke matrix once remaining legacy runtime modules are retired. |

## Milestone Rollup (Completed / Partial / Remaining)

### Completed

- Bridge runtime path removed (`src/lib/bridge.ts` absent).
- Typed `tauriClient` boundary active (`src/lib/tauri/client.ts`) with generated command/event bindings.
- Svelte app shell + mount flow in place (`src/App.svelte`, `src/main.ts`).
- Harness runtime available for isolated component iteration (`src/harness-main.ts`, `src/HarnessApp.svelte`).
- Existing guardrail: banned bridge imports (`scripts/check-no-bridge-imports.sh`).
- `jobControls` moved from imperative DOM wiring to reactive island/store boundary (`src/ui/jobControls.ts`, `src/ui/jobControls/state.svelte.ts`, `src/ui/jobControls/JobControlsIsland.svelte`).
- Guardrail ratchet applied for migrated lane: `scripts/check-no-imperative-dom-runtime.sh` now scans `jobControls` runtime paths.

### Partial

- UI islands are present, but many runtime features still use imperative DOM orchestration modules under `src/ui/**`.
- Metadata intent semantics are retained in boundary compile flows (`set|clear|noop`), but full store-driven runtime convergence is not complete.
- Status/processing flow is partly modernized but not fully detached from legacy render/event wiring.

### Remaining

- Complete migration of runtime feature modules from imperative DOM orchestration to reactive Svelte/store-first flows.
- Expand guardrails from "no bridge" to "no new imperative DOM patterns" on defined runtime paths, then ratchet stricter as legacy files retire.
- Close with full quality gates (`scripts/checks.sh standard` + `scripts/checks.sh package`) and merge readiness validation.

## Guardrail intent during partial state

This branch is intentionally in a transitional architecture state. Guardrails should:

1. Prevent new regression vectors now (no bridge resurrection, no new imperative DOM in newly migrated runtime surfaces).
2. Avoid blocking known legacy modules that are already tracked as migration debt.
3. Ratchet stricter as each legacy module is retired.

## Acceptance criteria for "zero-legacy" closure

- `src/ui/**` runtime paths no longer depend on imperative DOM orchestration modules.
- App-level save/process orchestration lives in reactive Svelte/store boundaries rather than direct `document.*` wiring.
- Runtime IPC/event access remains centralized via `src/lib/tauri/client.ts`.
- Guardrails in `scripts/checks.sh` enforce both:
  - no bridge usage
  - no banned imperative DOM patterns on the designated runtime path set
- `scripts/checks.sh standard` and `scripts/checks.sh package` pass on merge candidate head.
