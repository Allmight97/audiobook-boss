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
| 3. Wave 1 (encoder/output/job/cover) | partial | Svelte islands exist (`src/ui/*/*Island.svelte` for encoder/output/job/cover); `jobControls` runtime path is store/island-driven (`src/ui/jobControls.ts`, `src/ui/jobControls/state.svelte.ts`, `src/ui/jobControls/JobControlsIsland.svelte`); cover-art runtime UI state (image/message/loading/url input) is now island/state-driven (`src/ui/coverArt/CoverArtIsland.svelte`, `src/ui/coverArt/state.svelte.ts`, `src/ui/coverArt.ts`); output size updates are now decoupled from output-panel DOM listeners via encoder-originated events (`src/ui/encoderPanel/logic.ts`, `src/ui/outputPanel/handlers.ts`, `src/ui/outputPanel/dom.ts`). | Retire remaining imperative orchestration in `src/ui/encoderPanel/**` and `src/ui/outputPanel/**` (DOM cache + listener model), and finish cover-art residual runtime seams (`src/ui/coverArt.ts` drop-bound query + module-level orchestration). |
| 4. Wave 2 (import/list/metadata/tag/lookup) | partial | `metadataLookup` runtime path is store/island-driven (`src/ui/metadataLookup.ts`, `src/ui/metadataLookup/state.svelte.ts`, `src/ui/metadataLookup/MetadataLookupIsland.svelte`); tag preview now reads from metadata preview state (`src/ui/metadataForm/previewState.svelte.ts`, `src/ui/tagPreview.ts`); file import interactions are island/state-owned (`src/ui/fileImport/FileImportIsland.svelte`, `src/ui/fileImport/state.svelte.ts`, `src/ui/fileImport/handlers.ts`); file-list sort/clear + list interaction listeners are island-owned (legacy binder removed from `src/ui/fileList/index.ts`; delegated listeners are bound in `src/ui/fileImport/FileImportIsland.svelte` via `src/ui/fileList/events.ts` handler exports); file-list row rendering moved into Svelte markup with view-state sync (manual runtime `createElement`/`innerHTML` diff path replaced in `src/ui/fileList/dom.ts` + `src/ui/fileList/viewState.svelte.ts` + `src/ui/fileImport/FileImportIsland.svelte`); metadata form island now calls typed handlers directly instead of document-level custom-event bridging (`src/ui/metadataForm.ts`, `src/ui/metadataForm/MetadataFormFieldsIsland.svelte`). | Complete migration away from remaining imperative file-list/action seams (`src/ui/fileList/actions.ts` status DOM writes + legacy helper exports in `src/ui/fileList/dom.ts`/`src/ui/fileList/events.ts`) and finish full metadata form convergence. |
| 5. Processing/status/save flow | partial | Status panel island exists (`src/ui/statusPanel/StatusPanelIsland.svelte`) and queue/progress flow is typed through `tauriClient.listen`; metadata save button trigger no longer uses App-level `getElementById(...).addEventListener(...)`, and global save hotkey now uses `<svelte:window>` binding (`src/App.svelte`, `src/ui/metadataForm.ts`, `src/ui/metadataForm/MetadataFormFieldsIsland.svelte`); status process/cancel button click ownership moved to island handlers (`src/ui/statusPanel/StatusPanelIsland.svelte`, `src/ui/statusPanel/logic.ts`); preview split-button orchestration moved into App Svelte handlers (`src/App.svelte`) and legacy status DOM binder was removed (`src/ui/statusPanel/events.ts`); status cover-art rendering fallback path is removed in favor of view-state ownership (`src/ui/statusPanel/dom.ts`, `src/ui/statusPanel/viewState.svelte.ts`). | Finish removing remaining status legacy renderer/orchestrator surfaces (`src/ui/statusPanel/dom.ts` progress/status/job-list fallback path), then continue shrinking App-level save/status direct DOM access. |
| 6. Legacy retirement + enforcement | partial | `src/lib/bridge.ts` is retired; boundary is now `src/lib/tauri/client.ts`; no-bridge script wired in checks; no-imperative guard now includes migrated `jobControls`, `metadataLookup`, `tagPreview`, `fileImport`, `fileList/index`, and `metadataForm` runtime paths. | Expand no-new imperative DOM policy to each additional feature path as modules are migrated. |
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
- `metadataLookup` moved from imperative DOM rendering/listeners to reactive store/island flow (`src/ui/metadataLookup.ts`, `src/ui/metadataLookup/state.svelte.ts`, `src/ui/metadataLookup/MetadataLookupIsland.svelte`).
- Metadata lookup trigger is now Svelte-owned (`src/ui/metadataForm/MetadataFormFieldsIsland.svelte`) instead of module-attached DOM listener.
- Guardrail ratchet extended: `scripts/check-no-imperative-dom-runtime.sh` now scans `metadataLookup` runtime paths.
- `tagPreview` migrated from input-level DOM listeners to metadata preview store reads (`src/ui/tagPreview.ts`, `src/ui/metadataForm/previewState.svelte.ts`).
- File import drop-zone/error state is now reactive (`src/ui/fileImport/state.svelte.ts`) with island-owned interaction handlers (`src/ui/fileImport/FileImportIsland.svelte`, `src/ui/fileImport/handlers.ts`).
- Legacy file-list control binder removed (`src/ui/fileList/index.ts` no longer binds `DOMContentLoaded` + dataset-guarded listeners); sort/clear clicks are island-owned.
- File-list interaction listener registration is island-owned via delegated Svelte bindings (`src/ui/fileImport/FileImportIsland.svelte`) with pure handler exports in `src/ui/fileList/events.ts`; legacy DOM listener bootstrap in `fileList/events.ts` is now no-op compatibility.
- File-list runtime row rendering now flows through Svelte markup + view state (`src/ui/fileImport/FileImportIsland.svelte`, `src/ui/fileList/viewState.svelte.ts`) instead of imperative `innerHTML` patching in the runtime path.
- Metadata form no longer relies on document-level custom-event bridge; island events call typed handlers directly (`src/ui/metadataForm.ts`, `src/ui/metadataForm/MetadataFormFieldsIsland.svelte`).
- Metadata save trigger now flows through typed metadata-form callback wiring instead of App-level save button DOM listener (`src/App.svelte`, `src/ui/metadataForm.ts`, `src/ui/metadataForm/MetadataFormFieldsIsland.svelte`).
- Status panel process/cancel buttons are now island-owned (`src/ui/statusPanel/StatusPanelIsland.svelte`) with typed logic entry points (`src/ui/statusPanel/logic.ts`) instead of DOM-bound process/cancel event registration.
- Status panel job-list and cover-art rendering now flow through Svelte view state (`src/ui/statusPanel/viewState.svelte.ts`, `src/ui/statusPanel/viewTypes.ts`, `src/ui/statusPanel/StatusPanelIsland.svelte`) with compatibility fallback retained in `src/ui/statusPanel/dom.ts` for non-island/test contexts.
- Cover-art area interaction wiring (click-to-load, clear button click, hover, drag visuals) now lives in `src/ui/coverArt/CoverArtIsland.svelte`, reducing imperative listener setup in `src/ui/coverArt.ts`.
- Preview split-button behavior now lives in `App.svelte` Svelte handlers, and `src/ui/statusPanel/events.ts` is reduced to typed Tauri event subscriptions (legacy DOM preview binder removed).
- Cover-art runtime image/message/loading/url-input UI state is now reactive via `src/ui/coverArt/state.svelte.ts` + `src/ui/coverArt/CoverArtIsland.svelte`, and `src/ui/coverArt.ts` no longer mutates those UI elements directly.
- Status-panel cover-art fallback DOM renderer was removed; `displayCoverArt/resetArtThumbnail` now update reactive view state only (`src/ui/statusPanel/dom.ts`).
- Output-panel runtime no longer binds encoder `<select>` controls directly; encoder changes now emit `abb:encoder-settings-changed` from `src/ui/encoderPanel/logic.ts`, and output size refresh consumes that event (`src/ui/outputPanel/handlers.ts`, `src/ui/outputPanel/dom.ts`).
- Output-panel initial state hydration no longer scrapes encoder DOM controls; initialization is now limited to output naming toggles (`src/ui/outputPanel/state.ts`).
- Guardrail ratchet further extended: `scripts/check-no-imperative-dom-runtime.sh` now scans `tagPreview`, `fileImport`, `fileList/index`, and `metadataForm` runtime paths.

### Partial

- UI islands are present, but core runtime paths (`statusPanel/**`, `encoderPanel/**`, `outputPanel/**`) still contain imperative fallback/orchestration code; file-list runtime rendering is now island/state-driven with residual legacy helpers still present.
- Metadata intent semantics are retained in boundary compile flows (`set|clear|noop`), but full runtime convergence is still blocked by remaining status fallback/orchestrator seams and wave-1 legacy lanes.
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
