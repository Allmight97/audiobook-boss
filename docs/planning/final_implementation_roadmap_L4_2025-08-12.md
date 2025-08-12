## L4 Implementation Roadmap — Final (2025-08-12)

Reference: see audit for rationale and evidence: `../reports/l4_audit_and_updated_roadmap_draft_2025-08-12.md`.

### Phase P0 — E2E stability and user-facing fixes

- **P0.1 Cover art end-to-end in processing**
  - Backend: In `src-tauri/src/audio/processor/finalize.rs` `write_metadata_stage`, if `metadata.cover_art.is_some()`, call `crate::metadata::writer::write_cover_art(merged_output, bytes)` after `write_metadata`.
  - Frontend: In `src/ui/statusPanel/logic.ts` `getCurrentMetadata()`, include `cover_art` if present from `src/ui/coverArt.ts` (e.g., exported getter). Ensure `invoke('process_audiobook_files', { metadata })` receives it.
  - Verification: Add a unit test for `write_cover_art` (temp file) and an integration test that processes a file with cover art then reads it back with `read_audio_metadata` to assert a non-empty `cover_art`.

- **P0.2 Consistent cancellation semantics (backend event + UI)**
  - Backend: Emit `window.emit("processing-progress", { stage: "cancelled", ... })` when cancellation is detected before early returns in both engines:
    - Shell path: `src-tauri/src/audio/progress_monitor.rs` before returning `Err` in `check_cancellation_and_kill_context` and related paths.
    - ffmpeg-next path: guard points in `src-tauri/src/audio/media_pipeline.rs` where `ctx.context.is_cancelled()` returns early.
  - Frontend: In `src/ui/statusPanel/logic.ts` `handleCancel()`, stop setting stage to `cancelled`. Instead, show a local "Cancellation requested…" message and wait for the backend `cancelled` event to transition.
  - Verification: Manual click test + integration test that asserts cancel produces a `cancelled` event and stops processing without zombie processes.

- **P0.3 Preserve custom cover art across file selection**
  - Frontend: In `src/ui/coverArt.ts`, track `hasCustomCoverArt` (set on manual load, cleared on `clearCoverArt`). In `src/ui/fileList/actions.ts` `populateMetadataForm`, only call `setCoverArt(metadata.cover_art || null)` if `!hasCustomCoverArt`.
  - Verification: Manual UI test switching files does not overwrite manually loaded art; Clear button visibility toggles correctly.

- **P0.4 Deprecate old roadmap doc**
  - Mark `docs/planning/consolidated-roadmap.md` as deprecated and point readers to this file as the source of truth.

### Phase P1 — Maintainability and engine flip preparation

- **P1.1 Gate legacy adapters and remove dead-code allows**
  - Add a `legacy-adapters` feature (or align with `not(feature = "safe-ffmpeg")`) and gate `src-tauri/src/audio/processor/legacy.rs` and any deprecated adapters accordingly.
  - Remove `#![allow(dead_code)]` from `audio/*` where code is covered by features or used; retain only in test-only contexts.
  - Verification: `cargo clippy -- -D warnings` is clean for default and `--features safe-ffmpeg`.

- **P1.2 Event and stage type unification**
  - Frontend: Remove unused `merging` from `src/types/events.ts` and `src/ui/statusPanel/logic.ts` stage union if not emitted. Keep `converting`, `writing`, `completed`, `failed`, `cancelled`.
  - Ensure all progress messages flow through `ProgressEmitter` on the backend; minimize string drift.

- **P1.3 Default engine flip prep**
  - Optional alias: Introduce `type DefaultProcessor = ...` in a dedicated module, and use it in `execute.rs` for selection clarity while maintaining current `#[cfg(feature = "safe-ffmpeg")]` behavior.
  - Ensure both engines build and pass tests; document selection in code comments.

- **P1.4 ffmpeg-next internals cleanups**
  - In `src-tauri/src/audio/media_pipeline.rs`, move `stream_index`/`file_index` into the pipeline context to eliminate `#[allow(clippy::too_many_arguments)]` and keep functions < 60 LOC.
  - Verification: `cargo clippy` clean with feature on.

### Phase P2 — Cleanup, CI guards, parity and packaging

- **P2.1 Legacy removal and packaging simplification**
  - After validating engine flip readiness, delete or feature-gate `src-tauri/src/ffmpeg/*`, `src-tauri/src/audio/progress_monitor.rs`, and concat-file creation when building with `safe-ffmpeg`.
  - Update Tauri packaging to remove external FFmpeg when not needed (Apple Silicon only per project policy).

- **P2.2 CI and lint guards**
  - Add CI to fail on `allow(dead_code)` outside tests and run clippy with `-D warnings` for both default and `safe-ffmpeg` builds.

- **P2.3 Parity and performance checks**
  - Add a parity test comparing output properties (bitrate/quality tolerance) for shell vs ffmpeg-next (while legacy still available).
  - Introduce baseline performance measurements; optional optimization passes for ffmpeg-next throughput.

### Phase P3 — Extension hooks (from L6 design note)

- **P3.1 Processor extension seams**
  - Document and stabilize extension points around `finalize` (e.g., normalization, chapterization), ensuring `MediaProcessor` boundary and `ProgressEmitter` remain central.
  - No functional change required; provides a clear path for future stages.

### Verification checklist per phase
- Build/lint: `cargo clippy -- -D warnings` (default and `--features safe-ffmpeg`).
- Tests: `cargo test` (default and `--features safe-ffmpeg`).
- Frontend: Basic manual run `npm run tauri dev` to verify progress/cancel, cover art persistence and writing.


