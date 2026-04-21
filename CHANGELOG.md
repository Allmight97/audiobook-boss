# Changelog

All notable changes to AudioBook Boss™ will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed

## [1.0.9] - 2026-04-20

Focused contract-truth release that closes the remaining metadata proof gaps,
hardens fallback sunset enforcement, and reduces handwritten encoder boundary
drift at the Tauri edge.

### Changed

- Preview and full metadata proof now explicitly cover source-derived cover art
  preservation, multi-input chapter behavior, external-FDK preview parity, and
  ffmpeg-path `track` / `disk` reads.
- TypeScript encoder/process boundary types now derive from generated Tauri
  bindings more directly, keeping UI-only wrappers explicit while making Rust
  contract drift fail loudly.

### Fixed

- Fallback policy expiry and renewal checks now validate real calendar dates
  instead of accepting impossible `YYYY-MM-DD` values.
- Preview metadata behavior is now proven against both native and external
  processing paths, including exact cover-art byte preservation and truthful
  preview chapter suppression.
- FFmpeg-only and forced-fallback metadata reads now hydrate `track` and `disk`
  truthfully instead of leaving that proof limited to the mp4ameta path.

## [1.0.8] - 2026-04-20

Release follow-through for the durable output-contract branch, plus the final
status-panel and collision-dialog refinements needed to ship it cleanly.

### Added

- Existing-file collision preflight and one batch-wide resolution flow with
  explicit overwrite, keep-existing, and skip-existing choices before work
  starts.

### Changed

- Decoder/toolchain routing now uses stable decoder IDs at runtime while
  keeping friendly labels as display-only UI text.
- Preview and final outputs now share one backend-owned output plan and
  finalization contract, including `.preview.m4b` path resolution.
- Collision dialog copy and layout were tightened for clearer existing-file
  review without changing policy behavior.
- `track` and `disk` remain readable for compatibility but are no longer part
  of ABB's supported writable metadata contract.

### Fixed

- Existing output files are no longer replaced unless the user explicitly
  selects that policy, and finalization now re-checks destination occupancy
  before commit.
- Skipped batch rows now emit truthful terminal progress, all-skipped runs no
  longer surface as success, and skipped rows are preserved if cancellation
  arrives later.
- Hidden env-driven preview fallback was removed so normal processing no longer
  drifts into preview behavior without an explicit request.
- Frontend progress/status types now include the `skipped` terminal wire stage,
  keeping the TypeScript build aligned with regenerated Rust bindings.

## [1.0.7] - 2026-04-19

Focused contract-hardening release that makes output collision handling explicit
and finalization-aware, stabilizes external decoder routing, and closes the
in-process tail-loss gap in ABB's native pipeline.

### Added

- Backend output preflight planning plus a batch collision-resolution dialog
  with explicit replace, rename, and skip policies before processing starts.
- Skipped-result reporting and harness coverage for collision-dialog behavior
  and preview-path parity.

### Changed

- Output planning now flows through one backend-owned resolver across preview,
  queueing, execution, and finalization, including `.preview.m4b` artifacts.
- External decoder routing now uses stable decoder IDs instead of friendly UI
  labels, with toolchain validation that fails preflight when the chosen ffmpeg
  cannot honor the requested decoder.
- `track` and `disk` remain readable for compatibility but are no longer part
  of ABB's supported writable metadata contract.

### Fixed

- Existing destinations are no longer replaced unless the user selected an
  explicit collision policy, and source/destination overlap now hard-fails in
  backend planning.
- Preview output naming now matches the real preview artifact path, including
  finalization and fallback flows.
- The native in-process pipeline now drains decoder tail audio correctly and
  flushes accumulator tail samples exactly once, covering both loss and
  duplication regressions.

## [1.0.6] - 2026-04-18

Focused audiobook-edge-case fix release that closes a real xHE-AAC decode
handoff bug, restores large embedded cover-art rendering, and brings output
path behavior closer to the actual processing results ABB now emits.

### Changed

- External FDK processing now carries ABB's selected AAC-family input decoder
  into the subprocess ffmpeg command instead of letting that handoff drift.
- Existing output destinations now resolve to the requested path by default,
  while same-batch naming conflicts are still deconflicted separately.

### Fixed

- xHE-AAC / USAC inputs that validated against Apple AAC no longer fall back to
  ffmpeg's default external decode path during Auto(FDK AAC) processing.
- Large embedded cover art from imported sources now renders reliably in the UI
  without overflowing the old byte-to-data-URL conversion path.
- The Star Trek / Libation regression investigated in this thread now produces
  clean full-length output on the corrected external decoder path.

## [1.0.5] - 2026-04-17

Focused import-workflow release that closes the single-file re-add trap in ABB.
`Add audio files` and file-drop now behave coherently for iterative imports, and
the UI verification surface now proves that behavior instead of masking it.

### Added

- File-management harness coverage for add-while-populated behavior, plus mock
  dialog/analyzer paths that can distinguish append from replace during UI
  verification.

### Changed

- Status and file-list state now stay truthful under queued batches, fresh file
  imports, and ambiguous thumbnail lookups.
- The Tauri client boundary now uses typed command/event adapters instead of the
  old duplicate metadata-save alias and broader cast-heavy paths.
- Backend terminal progress cleanup now preserves failure `jobId` context
  without degrading cancellation into failed batch results.
- Tooling/docs cleanup pass: shared release shell helpers, cheaper release
  fixture setup, deduped context-surface lists, and compressed historical
  release notes.

### Fixed

- `Add audio files` and file-drop now append unseen audio files into a populated
  list instead of replacing the current import set on repeated adds.
- Additive imports now preserve pending metadata drafts, keep selection stable
  for pure append operations, and ignore duplicate-only batches with a transient
  status instead of mutating the list.
- **Status-panel terminal regression coverage now includes all terminal
  aggregates**. Failed and cancelled terminal aggregates, not just completed
  ones, are covered against stale `currentFile` / `etaSeconds` leakage, and
  batch terminal lifecycle tests now assert only the highest-priority toast
  fires.

### Removed
 
## [1.0.4] - 2026-04-16

Internal state-shape tightening release that continued the "make impossible
states unrepresentable" pass started in 1.0.3. Three boundary/UI refactors,
no wire-format or user-visible behavior changes.

### Changed

- `unwrapGeneratedResult` now accepts only the canonical specta `Result`
  shape, keeps bare values unchanged, and reduces `UnwrapGeneratedResult<T>`
  to three branches (`src/lib/tauri/appError.ts`, `src/lib/tauri/client.ts`).
- Cover art inline status is now a `CoverArtMessage` union instead of the
  `messageText` / `messageVariant` / `messageVisible` triple, and
  `showCoverArtMessage` / `clearCoverArtMessage` are exported for lifecycle
  tests (`src/ui/coverArt/state.svelte.ts`, `src/ui/coverArt/CoverArtIsland.svelte`).
- `ProcessingStatus` is now a `stage`-keyed discriminated union where only
  active stages carry `currentFile` / `etaSeconds`, and the factory and call
  sites route through that shape (`src/ui/statusPanel/state.ts`,
  `src/ui/statusPanel/controller.ts`).

### Added

- New unit coverage for the three seams above:
  - `src/lib/tauri-client.test.ts` pins canonical success/error wrapping,
    bare-value passthrough, and the `status: 'pending'` misclassification guard.
  - `src/ui/coverArt/__tests__/coverArtMessage.test.ts` covers show, auto-dismiss,
    and clear behavior for the inline message timer.
  - `src/ui/statusPanel/__tests__/progressAggregator.test.ts` verifies active
    progress fields do not leak into terminal aggregates.

### Deferred

Intentionally out of scope for this release (tracked for a successor plan):

- `normalizeProcessResult` inner `results` walk / `normalizeAppError(entry.error)`
  pattern in `src/lib/tauri/normalizers.ts`.
- Cleanup of the unused `isProcessingProgressEvent` runtime guard in
  `src/types/events.ts`.

## [1.0.3] - 2026-04-15

Internal seam-tightening release. No user-visible behavior changes; the
`processing-progress` wire format stayed identical, so older frontend builds
remained compatible with this backend.

### Changed

- IPC seam types were tightened end-to-end: Rust `EventStage`, the
  `From<&ProcessingStage>` mappings, regenerated TS bindings, stricter
  normalizers, and a simpler `src/types/events.ts` now agree on the same
  typed wire stage.

### Removed

- Dead TypeScript type definitions: `ProcessingProgress` and `ProcessingStage`
  from `src/types/audio.ts`, plus `MetadataResult`, `WriteMetadataParams`, and
  `WriteCoverArtParams` from `src/types/metadata.ts`.

### DX / Documentation

- Boundary guidance was refreshed in `.agents/skills/job-registry-and-progress/SKILL.md`,
  `src/types/events.ts`, and `src/lib/tauri/normalizers.ts` so the current
  Rust-authoritative wire stage model is documented in one place.

## [1.0.2] - 2026-03-31

Supply-chain install hardening release after the March 31, 2026 axios incident review.

### Changed

- **Dependency Intake Hardening**
  - Enforce lockfile-based Bun installs in CI by switching the perf workflow from `bun install` to `bun ci`.
  - Add a repo-level Bun release-age gate (`bunfig.toml`) so fresh dependency resolutions skip packages published within the last 7 days.

## [1.0.1] - 2026-02-22

Metadata lookup hardening release for legacy-title coverage and no-result recovery UX.

### Added

- **Metadata Lookup**
  - Add OpenLibrary as a selectable metadata source for online lookup.
  - Add a no-results modal CTA (`Use Manual Entry`) that closes lookup and focuses the main metadata title field for faster manual completion on rare/legacy editions.

### Changed

- **Metadata Lookup Resilience**
  - ASIN fast-path now respects selected source intent (OpenLibrary-only lookups skip Audnexus ASIN direct resolution).
  - Merge behavior preserves Audnexus-first ordering while reducing cross-source duplicate title/author matches.
- **DX / Contract Guardrails**
  - Introduce change-aware local IPC binding drift checks (`scripts/check-generated-bindings.sh --mode local`) to reduce iteration churn during non-contract edits.
  - Add hook-based binding auto-sync/stage flow in `.githooks/pre-commit` for staged Rust IPC contract changes.
  - Add explicit scripts for strict/local/sync binding workflows: `bindings:check`, `bindings:check:local`, `bindings:sync`.

### Fixed

- **Metadata Lookup**
  - Online lookup now returns a real error when all selected providers fail, instead of silently surfacing an empty-result state.
  - OpenLibrary lookups now request `description` in search fields so mapped descriptions can populate when available.

## [1.0.0] - 2026-02-20

Stable architectural milestone: zero-legacy IPC boundary with Svelte app shell.

### Added

- **IPC Boundary**
  - Centralized `tauriClient` (`src/lib/tauri/client.ts`) as canonical command/event interface
  - Generated TypeScript bindings via tauri-specta with drift detection
  - Normalizers for nullish/event-shape handling at boundary (`src/lib/tauri/normalizers.ts`)

- **Svelte Architecture**
  - App shell with reactive state model (`src/App.svelte`, `src/main.ts`)
  - Svelte islands for status panel, file list, cover art, metadata form, job controls, file import, metadata lookup
  - Reactive view-state modules (`.svelte.ts`) for island data binding
  - Component harness for isolated development (`src/HarnessApp.svelte`, `src/harness-main.ts`)

- **Guardrails**
  - `scripts/check-no-bridge-imports.sh` — blocks bridge resurrection
  - `scripts/check-no-imperative-dom-runtime.sh` — blocks new imperative DOM in migrated paths
  - `scripts/check-no-legacy-test-contracts.sh` — blocks legacy test contract patterns

- **Testing**
  - `@testing-library/svelte` integration for component testing
  - Colocated test strategy with reactive state assertions

### Changed

- **Architecture**
  - Retired `src/lib/bridge.ts` — all IPC now through `tauriClient`
  - Retired `src/lib/mocks.ts` — test mocking simplified
  - Status panel moved from imperative DOM to reactive view-state ownership
  - File-list control state moved to reactive bindings
  - Metadata save flow uses transient status API
  - Output config processing uses canonical `readOutputConfigForProcessing()`

- **Metadata Intent**
  - Explicit patch semantics (`set|clear|noop`) preserved at boundary
  - Clear intent compiles to Rust sentinels at normalizers, not scattered callsites

### Removed

- `src/lib/bridge.ts` — legacy IPC wrapper
- `src/lib/mocks.ts` — legacy test mocking layer
- `EncoderSettingsProvider` global coupling
- Legacy status-panel DOM fallback renderers
- Imperative row builders from `src/ui/fileList/dom.ts`
- `index.html` UI structure — now in `src/App.svelte`

## [0.1.0] - 2025-12-27

Initial development release with core audiobook processing functionality.

### Added

- **Audio Processing**
  - M4B audiobook creation from MP3, M4A, M4B, and FLAC source files
  - AAC encoding via ffmpeg-next with configurable bitrate and channels
  - Chapter marker preservation from source files
  - Progress tracking with real-time UI updates

- **Metadata Management**
  - Read/write metadata for MP4/M4B files via mp4ameta
  - Support for title, author, narrator, series, and series position
  - Cover art embedding with JPEG/PNG support
  - Audiobookshelf and Apple Books compatible tagging

- **Batch Processing**
  - Process multiple audiobooks in parallel
  - Configurable concurrency limits via job registry
  - Per-job cancellation support
  - Batch metadata application with defaults toggle

- **User Interface**
  - Drag-and-drop file import
  - Metadata editing panel with cover art preview
  - Processing progress display with time estimates
  - Dark theme support

- **Build & Distribution**
  - macOS app bundle generation
  - DMG installer creation script
  - Apple Silicon (ARM64) primary target

### Fixed

- Cover art reads for MP4/M4B files now work correctly
- Channel/sample rate probe failures now fail fast with clear errors
- Decoder flush no longer logs duplicate entries

### Changed

- Migrated from shell FFmpeg to ffmpeg-next Rust bindings
- Refactored audio pipeline to v2 configuration architecture
- Encoder settings now use type-safe configuration objects
