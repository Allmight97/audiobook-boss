# Changelog

All notable changes to AudioBook Boss™ will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Removed

## [1.0.4] - 2026-04-16

Internal state-shape tightening release, continuing the "make impossible
states unrepresentable" pass started in 1.0.3. Three focused refactors along
the IPC seam and UI state layer. No wire-format or user-visible behavior
changes.

### Changed

- **`unwrapGeneratedResult` narrowed to the canonical specta `Result` shape**
  (`src/lib/tauri/appError.ts`, `src/lib/tauri/client.ts`). Rewrite the
  function from a 4-branch duck-type across `{Ok,Err}` / `{ok: boolean}` /
  `{status, data|error}` shapes down to a tight three-way discrimination:
  specta success returns `.data`, specta error throws via `normalizeAppError`,
  everything else passes through unchanged (bare-value commands like
  `get_max_concurrent_jobs` and `list_available_encoders`). Replace the loose
  `isResultRecord` (had-a-`status`-key) with `isSpectaResult` that checks
  `status === 'ok' | 'error'` explicitly — removes the misclassification
  hazard where a future domain type with an unrelated `status` field would
  be treated as a `Result` envelope. Simplify the matching
  `UnwrapGeneratedResult<T>` conditional type from 7 branches to 3.
- **Cover art inline message modeled as a discriminated union**
  (`src/ui/coverArt/state.svelte.ts`, `src/ui/coverArt/CoverArtIsland.svelte`).
  Replace the `messageText` / `messageVariant` / `messageVisible` triple with
  a single `CoverArtMessage` union of `{ kind: 'hidden' }` /
  `{ kind: 'error'; text }` / `{ kind: 'success'; text }`. Impossible states
  (`messageVisible: false` with lingering text, unknown variant values) are
  now unrepresentable. Export `showCoverArtMessage` /
  `clearCoverArtMessage` from `src/ui/coverArt.ts` to enable first-class unit
  coverage of the 4-second auto-dismiss lifecycle.
- **`ProcessingStatus` modeled as a discriminated union**
  (`src/ui/statusPanel/state.ts`, `src/ui/statusPanel/controller.ts`).
  Replace the flat product `{ stage; percentage; message; currentFile?;
  etaSeconds? }` with a `stage`-keyed union where only the active-stage
  variant (derived from `EventStage` via
  `type ActiveEventStage = Exclude<EventStage, 'completed' | 'failed' |
  'cancelled'>`) carries `currentFile` / `etaSeconds`. Terminal and idle
  variants drop them at the type level — stale-progress-fields-on-terminal
  status is now type-impossible. Add a `buildStatus` factory that gates
  active extras on the discriminant; the four construction sites in
  `controller.ts` (`applyQueueSnapshot`, `updateAggregateUI`, `flushRender`,
  `requestCancelAll`) route through it. The `flushRender` path at
  `controller.ts:~416-425`, which previously copied `current_file` /
  `eta_seconds` from `latestProgressEvent` onto any stage, is now gated at
  the type level.

### Added

- **New unit coverage**
  - `src/lib/tauri-client.test.ts` — `unwrapGeneratedResult` tests for
    the canonical success/error paths, bare-scalar and bare-object
    passthrough, and an explicit misclassification guard
    (`{ status: 'pending', ... }` must fall through).
  - `src/ui/coverArt/__tests__/coverArtMessage.test.ts` — lifecycle of
    the cover-art inline message: show sets the right variant, 4-second
    auto-dismiss transitions to `{ kind: 'hidden' }`, and
    `clearCoverArtMessage` cancels the pending timeout so a subsequent
    show is not prematurely hidden by a stale timer. Per-case isolation
    via `vi.resetModules()` + dynamic imports (timer handle lives in
    module scope).
  - `src/ui/statusPanel/__tests__/progressAggregator.test.ts` — one
    targeted regression test pinning the core hazard: active progress
    event populates `latestProgressEvent.current_file` / `eta_seconds`,
    then a terminal event rolls the aggregate over, and the resulting
    `ProcessingStatus` must not leak the active-stage fields.

### Deferred

Intentionally out of scope for this release (tracked for a successor plan):

- `JobProgress.stage` refactor toward a stricter `EventStage`-based shape.
- `normalizeProcessResult` inner `results` walk / `normalizeAppError(entry.error)`
  pattern in `src/lib/tauri/normalizers.ts`.
- Cleanup of the unused `isProcessingProgressEvent` runtime guard in
  `src/types/events.ts`.

## [1.0.3] - 2026-04-15

Internal seam-tightening release. No user-visible behavior changes; the
`processing-progress` event wire format is unchanged (string values stay
identical), so older frontend builds remain compatible with this backend.

### Changed

- **IPC Seam Types (Rust → TypeScript)**
  - Replace stringly-typed `ProgressEvent.stage: String` with a typed
    `EventStage` enum in Rust (`src-tauri/src/audio/progress/mod.rs`),
    serialized snake_case via serde so the wire format is preserved.
  - Add `From<&ProcessingStage> for EventStage` so the three emission
    sites (`ProgressEmitter::emit_event`, `ProgressEmitter::emit_cancelled`,
    and `emit_terminal_failed_event` in `commands/audio_processing.rs`)
    derive the wire stage from the internal orchestration enum instead
    of duplicating string literals at every callsite.
  - Regenerate `src/lib/generated/tauri.ts`; the frontend now sees
    `stage: EventStage` (a real string-literal union) instead of
    `stage: string`.
  - Tighten the seven public normalizers in `src/lib/tauri/normalizers.ts`
    to accept their generated input types (`GeneratedAudiobookMetadata`,
    `GeneratedFileListInfo`, `GeneratedEncoderAvailability`,
    `GeneratedOnlineMetadataResult`, `GeneratedProcessCommandResult`,
    `GeneratedProgressEvent`, `GeneratedQueueEvent`) instead of `unknown`,
    and return `NullToOptionalDeep<T>` UI types — drift now surfaces at
    compile time rather than as a runtime cast.
  - Strengthen `normalizeNullish<T>` to return `NullToOptionalDeep<T>` so
    output `as` casts at every call site are no longer needed.
  - Simplify `src/types/events.ts`: re-export `EventStage` from the
    generated bindings, type `STAGES` as `{ [K in EventStage]: K }` so
    new Rust variants fail the TS build, collapse `ProcessingProgressEvent`
    to `NullToOptionalDeep<GeneratedProgressEvent>` (no more `Omit + remap`),
    and tighten the runtime `isProcessingProgressEvent` guard.

### Removed

- **Dead TypeScript Type Definitions**
  - Remove `ProcessingProgress` and `ProcessingStage` from
    `src/types/audio.ts` (PascalCase duplicate of the Rust internal enum
    with no consumers).
  - Remove `MetadataResult`, `WriteMetadataParams`, and `WriteCoverArtParams`
    from `src/types/metadata.ts` (the real IPC contract flows through
    `tauriClient` and `Result<T, AppErrorEnvelope>`; these standalone
    interfaces had zero callers).

### DX / Documentation

- **Boundary Guidance Refresh**
  - Update `.agents/skills/job-registry-and-progress/SKILL.md` to invert
    the stale "TS leads, Rust follows" framing — Rust `EventStage` is
    now the wire authority and TS only re-exports it. Document the
    distinction between `EventStage` (flat wire discriminator) and the
    internal Rust `ProcessingStage` (carries `Failed(String)`).
  - Refresh the file header on `src/types/events.ts` to drop the
    obsolete "Phase 0" framing and describe present-state intent.
  - Add a typed-input contract doc-block to `src/lib/tauri/normalizers.ts`
    so the symmetry between `normalize*` and `denormalize*` is
    discoverable from the file header.

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
