## L4 Implementation Roadmap — Final (2025-08-12)

Reference: see audit for rationale and evidence: `../reports/l4_audit_and_updated_roadmap_draft_2025-08-12.md`.

### Phase P0.5 — Tauri v2 foundation modernization

- **P0.5.1 Security permissions audit** ✅
  - Backend: Replace `src-tauri/capabilities/default.json` broad permissions (`core:default`, `dialog:default`, `opener:default`) with explicit least-privilege grants actually used by the UI.
    - Events (frontend): `core:event:allow-listen`, `core:event:allow-unlisten` (do not grant `allow-emit` to the frontend unless required).
    - Dialog: `dialog:allow-open`.
    - Path: avoid granting any `core:path:*` unless a specific command is used by the frontend (none required at present).
    - Opener: include `opener:allow-open-url` only if there is an actual UI feature invoking it; otherwise remove `opener:*` entirely.
  - Success criteria: `npm run tauri dev` starts without capability errors; smoke test file-open dialog and progress events with the narrowed capability set.

- **P0.5.2 Frontend API standardization** ✅
  - Frontend: Verify all TypeScript uses `@tauri-apps/api` imports (no `window.__TAURI__` references). Current code already follows this; keep it enforced.
  - Replace any stragglers if discovered: `window.__TAURI__.core.invoke` → `import { invoke } from '@tauri-apps/api/core'`, `window.__TAURI__.event.listen` → `import { listen } from '@tauri-apps/api/event'`.
  - Success criteria: No `window.__TAURI__` references exist; imports resolve and app runs.

- **P0.5.3 Event constants definition** ✅
  - Frontend: Create constants in `src/types/events.ts` for the single progress event and for stage names to prevent string drift:
    - `export const EVENTS = { PROGRESS: 'processing-progress' } as const;`
    - `export const STAGES = { analyzing: 'analyzing', converting: 'converting', merging: 'merging', writing: 'writing', completed: 'completed', failed: 'failed', cancelled: 'cancelled' } as const;`
  - Backend: Align emitted stage strings to match frontend stages (rename `writing_metadata` → `writing` in the emitter mapping); optionally define a Rust-side constant for the event name.
  - Update all emit/listen and stage comparisons to use constants instead of string literals.
  - Success criteria: No hardcoded event strings remain; FE/BE stage names match (specifically `writing`).

### Phase P0 — E2E stability and user-facing fixes ✅

- **P0.1 Cover art end-to-end in processing** ✅
  - Backend: In `src-tauri/src/audio/processor/finalize.rs` `write_metadata_stage`, if `metadata.cover_art.is_some()`, call `crate::metadata::writer::write_cover_art(merged_output, bytes)` after `write_metadata`.
  - Frontend: In `src/ui/statusPanel/logic.ts` `getCurrentMetadata()`, include `cover_art` if present from `src/ui/coverArt.ts` (e.g., exported getter). Ensure `invoke('process_audiobook_files', { metadata })` receives it.
  - Success criteria: Add a unit test for `write_cover_art` (temp file) and an integration test that processes a file with cover art then reads it back with `read_audio_metadata` to assert a non-empty `cover_art`.

- **P0.2 Consistent cancellation semantics (backend event + UI)** ✅
  - Backend: Emit cancellation events using event constants from P0.5.3. In both engines emit `EVENTS.CANCELLED` when cancellation is detected before early returns:
    - Shell path: `src-tauri/src/audio/progress_monitor.rs` before returning `Err` in `check_cancellation_and_kill_context` and related paths.
    - ffmpeg-next path: guard points in `src-tauri/src/audio/media_pipeline.rs` where `ctx.context.is_cancelled()` returns early.
  - Frontend: In `src/ui/statusPanel/logic.ts` `handleCancel()`, stop setting stage to `cancelled`. Instead, show a local "Cancellation requested…" message and wait for the backend `cancelled` event to transition.
  - Success criteria: Manual click test + integration test that asserts cancel produces a `cancelled` event and stops processing without zombie processes. RUST_LOG=debug npm run tauri dev - confirmed working.

- **P0.3 Preserve custom cover art across file selection** ✅
  - Frontend: In `src/ui/coverArt.ts`, track `hasCustomCoverArt` (set on manual load, cleared on `clearCoverArt`). In `src/ui/fileList/actions.ts` `populateMetadataForm`, only call `setCoverArt(metadata.cover_art || null)` if `!hasCustomCoverArt`.
  - Success criteria: Manual UI test switching files does not overwrite manually loaded art; Clear button visibility toggles correctly. - Tested and confirmed working.

### Phase P1 — Maintainability and engine flip preparation

- **P1.1 Gate legacy adapters and remove dead-code allows** ✅
  - Add a `legacy-adapters` feature (or align with `not(feature = "safe-ffmpeg")`) and gate `src-tauri/src/audio/processor/legacy.rs` and any deprecated adapters accordingly.
  - Remove `#![allow(dead_code)]` from `audio/*` where code is covered by features or used; retain only in test-only contexts.
  - Success criteria: `cargo clippy -- -D warnings` is clean for default and `--features safe-ffmpeg`.
  - DONE: gating and dead-code cleanup across src-tauri/src/audio/*, added legacy-adapters feature (default-enabled), and ensured the legacy path remains available while enabling a non-legacy path when disabled. Build/lint/tests green for default and --features safe-ffmpeg

- **P1.2 Event and stage type unification** ✅
  - Frontend: Remove unused `merging` from `src/types/events.ts` and `src/ui/statusPanel/logic.ts` stage union if not emitted. Keep `converting`, `writing`, `completed`, `failed`, `cancelled`.
  - Backend: Ensure all progress messages flow through `ProgressEmitter` using event constants from P0.5.3; minimize string drift.
  - Success criteria: All events use centralized constants; no hardcoded event strings remain.
  
  **DONE** – Frontend removed `merging` (`src/types/events.ts`, `src/ui/statusPanel/logic.ts`); Backend removed dormant `ProcessingStage::Merging` and merging constants; all progress via `ProgressEmitter` (`PROGRESS_EVENT_NAME`). Build/lint/tests green for default and `--features safe-ffmpeg`.

  TODO: Highest-impact improvements (for later)
  - [ ] Shared stage schema generation (Rust → TS) to eliminate cross-language drift.
  - [ ] CI guard to fail on hardcoded `processing-progress` strings outside constants.
  - [ ] Remove legacy progress monitor when `safe-ffmpeg` is enabled (scheduled under P2.1).

- **P1.3 Default engine flip prep** ✅
  - Optional alias: Introduce `type DefaultProcessor = ...` in a dedicated module, and use it in `execute.rs` for selection clarity while maintaining current `#[cfg(feature = "safe-ffmpeg")]` behavior.
  - Ensure both engines build and pass tests; document selection in code comments.

- **P1.4 ffmpeg-next internals cleanups** ✅ (Completed 2025-08-14)
  - Refactored `media_pipeline.rs`: moved `stream_index` & `file_index` into `FramePipelineCtx` (`current_stream_index`, `current_file_index`).
  - Removed `#[allow(clippy::too_many_arguments)]` from `process_input_packets`; reduced arg surface across helper functions.
  - Centralized progress emission using context state; all affected functions remain < 60 LOC.
  - Verified with `cargo clippy --features safe-ffmpeg -D warnings` and full test suite.
  - (Optional next) Add a dedicated unit test for progress emission frequency (currently covered indirectly).

### Phase P2 — Cleanup, CI guards, parity and packaging

- **P2.1 Legacy removal and packaging simplification** ✅ (Completed 2025-08-14)
  - Feature-gated `src-tauri/src/ffmpeg/*` and `src-tauri/src/audio/progress_monitor.rs` to exclude from `safe-ffmpeg` builds
  - Both build configurations validated: `cargo build` (legacy) and `cargo build --features safe-ffmpeg` compile successfully
  - Enhanced build script warnings document external FFmpeg bundling when unused in safe-ffmpeg mode
  - Success criteria: Clean separation enables 100% shell removal in future without breaking existing functionality

- **P2.2 CI and lint guards** ✅ (Completed 2025-08-14)
  - CI workflow validates zero `#[allow(dead_code)]` outside test contexts via automated grep check
  - Clippy with `-D warnings` passes for both default and `safe-ffmpeg` configurations
  - Removed 15+ unnecessary dead code allows; added conditional allows for legitimately feature-gated code
  - Success criteria: Automated quality gates prevent accumulation of unused code and lint violations

- **P2.3 Parity and performance checks** — TODO
  - **Parity validation strategy:**
    - Implement cross-engine output comparison tests using identical input files
    - Verify bitrate accuracy within ±5% tolerance between shell and ffmpeg-next outputs
    - Compare audio duration precision and metadata preservation across engines
    - Validate chapter marker accuracy and cover art embedding consistency
  
  - **Performance measurement framework:**
    - Add `std::time::Instant` timing around core processing phases (parsing, concat, encode, finalize)
    - Implement memory usage tracking during large file processing (use `system-stats` crate)
    - Create benchmark harness with representative file sizes (5min, 30min, 3hr samples)
    - Log processing throughput metrics (MB/s, realtime factor) to structured output
    
  - **Optimization guidance for ffmpeg-next:**
    - Profile frame-by-frame processing to identify bottlenecks vs shell approach
    - Consider implementing parallel chunk processing for large audiobooks  
    - Evaluate memory buffer sizing for optimal throughput (test 64KB, 256KB, 1MB buffers)
    - Add configurable quality presets optimized for different use cases (speed vs quality)
    
  - **Regression testing:**
    - Establish baseline performance metrics for shell engine before migration
    - Create automated performance regression detection (>20% slowdown triggers CI failure)
    - Add memory leak detection for long-running operations
    - Test processing stability with edge cases (corrupted files, extremely long audiobooks)

### Phase P3 — Extension hooks (from L6 design note)

- **P3.1 Processor extension seams**
  - Document and stabilize extension points around `finalize` (e.g., normalization, chapterization), ensuring `MediaProcessor` boundary and `ProgressEmitter` remain central.
  - No functional change required; provides a clear path for future stages.

### Success criteria checklist per phase
- Build/lint: `cargo clippy -- -D warnings` (default and `--features safe-ffmpeg`).
- Tests: `cargo test` (default and `--features safe-ffmpeg`).
- Frontend: Basic manual run `npm run tauri dev` to verify progress/cancel, cover art persistence and writing.
- P0.5 specific: Verify modern Tauri API usage, proper permissions, and event constant usage throughout codebase.

### Phase 4 - (optional) Nice-to-haves to reach Level 5
**Consider these prior to docs in phase 5 as items to implement for public distribution post MVP**
  - From phase 0.5:
    - Add a brief Success criteria matrix doc covering: dev run, progress event smoke test, dialog open, and narrowed capabilities validation.
    - Audit and remove unused plugin capabilities (e.g., `opener:*`) and capture the minimal set in documentation.
    - Add `tracing` + subscriber setup for async observability on long-running operations.
    - Evaluate Vite v7 upgrade plan (compat notes, roll-back plan), schedule once compatible with Tauri 2 toolchain.
  - From phase 0:
    - Add unit tests for `write_cover_art` function covering edge cases (empty files, corrupted data, permission failures).
    - Create integration tests for complete E2E cover art flow: selection → processing → verification with `read_audio_metadata`.
    - Add comprehensive cancellation integration tests covering both shell and ffmpeg-next engines with zombie process detection.
    - Enhance error handling with user-friendly messages for cover art loading failures and processing errors.
    - Add performance optimization for large cover art files (lazy loading, compression, size limits).
  - From phase 1:
    - Pending Review ...
  - From phase 2:
    - Pending Review ...
  - From phase 3:
    - Pending Review ...

TODO Phase 5 - Create/update formal documentation
- [ ] Document complete cover art flow with usage examples in user-facing documentation.
- Tasks pending ...


