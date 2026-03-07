# StatusPanel Module Structure

Current status-panel ownership is mixed: rendering is largely island/view-state driven, but processing lifecycle truth still lives in the singleton `StatusPanel` controller. Treat this file as a module map, not as a claim that the next-pass de-hybridization is already complete.

## Current Runtime Posture

- `StatusPanelIsland.svelte` is the render host for the panel UI.
- `viewState.svelte.ts` owns the reactive view state used by the island.
- `logic.ts` still owns the real processing lifecycle through `StatusPanel`, `initStatusPanel()`, and `getStatusPanel()`.
- `processing.ts`, `events.ts`, `render.ts`, and `dom.ts` are support layers around that controller.

This means the module is cleaner than the earlier DOM-heavy model, but it is still a known remaining ownership seam. The planned steady state is store/service ownership without a singleton controller.

## Files Overview

### `/index.ts`

Current public API surface:

- `StatusPanel` class
- `initStatusPanel()` function
- `getStatusPanel()` function
- transient status helpers used by adjacent features

### `/logic.ts`

Current controller-owned runtime logic:

- processing lifecycle orchestration
- progress and queue listener installation
- job-map and timer ownership
- bridge between backend events and view-state/render helpers

### `/domain/`

Pure status-panel domain helpers:

- `jobKeys.ts` for stable per-job key derivation
- `queueState.ts` for queue snapshot state and terminal-state checks
- `aggregate.ts` for aggregate progress/stage derivation wrappers

### `/processing.ts`

Processing workflow helpers:

- backend invocation payload assembly
- metadata preparation for merge/batch
- processing lifecycle coordination helpers

### `/state.ts`

In-memory state helpers and types:

- processing status and per-job progress types
- aggregate progress calculations
- stage derivation

### `/events.ts`

Backend/event wiring:

- progress event subscription
- queue event subscription

### `/formatting.ts`

Message and data formatting:

- status and aggregate message formatting
- cover-art data URL conversion
- progress label parsing helpers

### `/services/`

Side-effect and boundary helpers:

- `artThumbnail.ts` for metadata reads and cover-art data URL extraction
- `fileLookup.ts` for file path lookup by name/index
- `progressThrottle.ts` for non-terminal progress throttling decisions

### `/render.ts` and `/dom.ts`

Legacy support layers that still help the singleton controller update the panel UI. These are candidates to shrink or disappear when processing state/actions become store-owned in the next de-hybridization pass.
