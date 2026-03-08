# StatusPanel Module Structure

Current status-panel ownership is still mixed: rendering is island/view-state driven, lifecycle truth lives in a typed `StatusPanelController`, and app-level access still flows through a singleton `StatusPanel` shell. Treat this file as a module map, not as a claim that the final store/service cutover is already complete.

## Current Runtime Posture

- `StatusPanelIsland.svelte` is the render host for the panel UI.
- `viewState.svelte.ts` owns the reactive view state used by the island.
- `controller.ts` owns the real processing lifecycle through `StatusPanelController`.
- `logic.ts` is the singleton/runtime shell that exposes `StatusPanel`, `initStatusPanel()`, and `getStatusPanel()`.
- `processing.ts`, `events.ts`, `render.ts`, and `dom.ts` are support layers around that controller.

This means the module is materially cleaner than the earlier private-internals model: lifecycle tests now target the public controller instead of `(panel as any)` access. It is still a known remaining ownership seam because singleton runtime access has not been retired yet. The planned steady state remains store/service ownership without a singleton shell.

## Files Overview

### `/index.ts`

Current public API surface:

- `StatusPanel` class
- `initStatusPanel()` function
- `getStatusPanel()` function
- transient status helpers used by adjacent features

### `/logic.ts`

Current runtime shell logic:

- singleton instance management
- app-facing `StatusPanel` wrapper methods
- trigger helpers used by the island and adjacent modules

### `/controller.ts`

Current controller-owned runtime logic:

- processing lifecycle orchestration
- progress and queue listener installation
- job-map and timer ownership
- cancel/reset behavior
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

Support layers that help the typed controller update the panel UI. These are still candidates to shrink or disappear when processing state/actions become store-owned in the next de-hybridization pass.
