### Migration Plan: Move processing to ffmpeg-next (type-safe FFmpeg)

#### Context & Goals
- Replace external `ffmpeg` process calls with the `ffmpeg-next` Rust bindings.
- Keep UX identical (same inputs/outputs, progress, metadata) with safer, testable code.
- Minimize risk via feature flag, parallel implementation, and fast rollback.

Non-goals (for this migration):
- Broad UI redesign; new codecs/presets; multi-threaded scheduling. These can follow after the migration.

References
- See `docs/reports/dependency-map.md` for call flow and ownership boundaries.
- Current boundary: `audio/media_pipeline.rs::MediaProcessor` (now implemented by `ShellFFmpegProcessor`).

#### Target Architecture
- Add `FfmpegNextProcessor` that implements `MediaProcessor` and becomes an alternate engine.
- Compilation gated by feature `safe-ffmpeg` (off by default initially). Retain the legacy shell path as default for easy rollback.
- Progress and cancellation stay in the `audio` layer; ffmpeg-next code emits periodic progress derived from decoded timestamps and total duration held in `MediaProcessingPlan`.
- Keep metadata write step unchanged (we still call `metadata::writer`).

#### Rollout Strategy (phased)
- P0 (enable parallel path; no default switch)
  1. Add deps (compile only when feature enabled) — DONE
     - `Cargo.toml` (root crate in `src-tauri/`):
       - `[features] safe-ffmpeg = []` (no default)
       - `[dependencies] ffmpeg-next = "^7"` (behind cfg when used)
       - `[build-dependencies]` unchanged
     - Doc the platform prerequisite: Homebrew `ffmpeg` dev libs are required to build `ffmpeg-next` (Apple Silicon only). Command: `brew install ffmpeg`.
  2. Create module `src-tauri/src/processing/ffmpeg_next.rs` (or `src-tauri/src/audio/ffmpeg_next.rs`) with `FfmpegNextProcessor` implementing `MediaProcessor` — BOOTSTRAPPED in `audio/media_pipeline.rs`:
     - `execute(&self, plan: &MediaProcessingPlan, ctx: &ProcessingContext) -> Result<()>` (async wrapper that runs blocking work in a dedicated thread if needed).
     - Pipeline (first pass): open each input via `ffmpeg_next::format::input`, find audio stream, decode packets and re-encode to AAC into an MP4/M4B container (libavcodec `aac`).
       - Sample rate: from `plan.settings` (explicit) or detected (`MediaProcessingPlan` already computed; reuse value). Channels: mono/stereo per `settings`.
       - Bitrate: set from `settings.bitrate` (kbps).
       - Produce single continuous output file (concatenate by sequentially appending encoded frames for each input).
     - Progress: compute total seconds from `plan.total_duration`; emit progress every N frames using accumulated PTS/time.
     - Cancellation: poll `ctx.is_cancelled()` and abort early with a clean shutdown.
  3. Wire a chooser without changing default behavior — DONE:
     - In `audio/processor.rs::merge_audio_files_with_context`, we instantiate `FfmpegNextProcessor` when compiled with `--features safe-ffmpeg`, else `ShellFFmpegProcessor`.
     - Default path remains legacy (shell), preserving behavior.
  4. Tests and lint — DONE
     - Feature-gated tests added at `src-tauri/tests/unit/audio/ffmpegnext_tests.rs` (use `media/01 - Introduction.mp3` when present).
     - Default build remains unchanged and green; feature-on build compiles and tests pass on macOS.
  5. Docs:
     - Build note: Enable `safe-ffmpeg` and ensure Homebrew FFmpeg dev libs on macOS (Apple Silicon). Command: `brew install ffmpeg`.

- P1 (flip-by-project-level setting; start retiring legacy)
  1. Feature flag scaffold in code paths:
     - Add `#[cfg(not(feature = "safe-ffmpeg"))] type DefaultProcessor = ShellFFmpegProcessor;`
     - Add `#[cfg(feature = "safe-ffmpeg")] type DefaultProcessor = FfmpegNextProcessor;`
     - Use `DefaultProcessor` in `merge_audio_files_with_context` so flipping Cargo feature changes the engine.
  2. Remove concat-file dependency from execution path when `safe-ffmpeg` is on:
     - Stop creating `concat.txt` under the new engine; derive input list directly from `plan.input_file_paths`.
  3. Deprecate legacy-only helpers (tag for removal; see checklist below):
     - Mark `ffmpeg::escape_ffmpeg_path`, `ffmpeg::format_concat_file_line`, `ffmpeg::command::*`, `media_pipeline::build_merge_command`, and `media_pipeline::execute_ffmpeg_with_progress_context` as `#[deprecated]` behind `cfg(not(feature = "safe-ffmpeg"))` and reference this plan.
  4. Progress parity & UI UX:
     - Maintain current stages and messages; ensure final events always fire.
  5. Optional: checksum verification of any bundled external binary becomes obsolete when the default is `safe-ffmpeg` (no external binary needed). Keep verification only for legacy builds.

- P2 (cleanup and removal once stable)
  1. Remove legacy shell code paths from the default build:
     - Delete or gate behind a `legacy-ffmpeg` feature the following (see checklist):
       - `src-tauri/src/ffmpeg/*` (module, command builder, path escape helpers)
       - `audio/progress_monitor.rs` (child process lifecycle)
       - `media_pipeline::build_merge_command` and `execute_ffmpeg_with_progress_context`
       - Concat file creation in `audio/processor.rs` (and any call sites)
     - Remove `bundle.externalBin` from `tauri.conf.json` for release artifacts when legacy is off.
  2. Update docs: remove concat escaping notes; move security notes to reflect new boundary.
  3. Expand tests: add more feature-on integration tests; remove legacy-only tests.

#### Detailed Tasks (with P0/P1/P2)
- P0
  - [X] Add Cargo feature `safe-ffmpeg` and conditional deps/imports.
  - [X] Implement `FfmpegNextProcessor` minimally to merge inputs → AAC in M4B container; emit progress; honor cancel.
  - [X] Wire non-default selection via compile-time feature (no behavior change by default).
  - [X] Add a small, feature-gated test using repository media.
  - [X] Update development docs and hand-off notes (this doc + hand-off updated).

#### P0 Implementation Notes
- Encoder: libavcodec AAC via ffmpeg-next; bitrate from settings (kbps), channels from settings, sample rate explicit or derived from first input when Auto.
- Container: mp4/m4b muxer selected by output path; continuous timeline by rescaling PTS and appending frames across inputs.
- Resampling: per-input resampler to target encoder format (rate/layout/sample fmt).
- Progress: computed from accumulated encoded samples vs `plan.total_duration`, emitted every ~200ms and clamped to converting range.
- Cancellation: checked each packet/frame; aborts cleanly before trailer write when requested.

#### How to run locally (feature-on)
- macOS setup (one-time): `brew install ffmpeg`
- Build and test (feature on) from `src-tauri/`:
  - `cargo test --features safe-ffmpeg`
  - Optionally: `cargo clippy --features safe-ffmpeg`

#### Next tasks (can proceed mostly in parallel)
- Implement core decode→encode loop in `FfmpegNextProcessor` (P0)
- Emit progress by accumulated PTS; plumb cancel checks (P0)
- Wire runtime selection behind feature with a `DefaultProcessor` alias (P1-prep)
- Add feature-gated integration test using small media sample (P0)

- P1
  - Switch default engine via `DefaultProcessor` type alias when building with `--features safe-ffmpeg`.
  - Stop writing concat files in the new engine; pass inputs directly.
  - Mark legacy helpers as deprecated under `cfg(not(feature = "safe-ffmpeg"))`.
  - Optional: keep bundled external ffmpeg only for legacy builds; document build matrix.

- P2
  - Remove legacy shell-based code and artifacts from default build; retain behind `legacy-ffmpeg` only if needed.
  - Clean up tests/docs; remove concat/escape references and progress-monitor process code.

#### Removal/Deprecation Checklist (legacy shell implementation)
Tag now (P1) and remove at P2 unless otherwise noted:
- `src-tauri/src/ffmpeg/mod.rs` (entire module) — includes `escape_ffmpeg_path`, `format_concat_file_line`, `locate_ffmpeg()`.
- `src-tauri/src/ffmpeg/command.rs` — FFmpegCommand builder and helpers.
- `src-tauri/src/audio/progress_monitor.rs` — only used for child process lifecycle.
- `src-tauri/src/audio/media_pipeline.rs`
  - `build_merge_command`
  - `execute_ffmpeg_with_progress_context`
- `src-tauri/src/audio/processor.rs`
  - `create_concat_file` and any concat-file call sites (only when `safe-ffmpeg` is the default)
- `src-tauri/tauri.conf.json`
  - `bundle.externalBin` entry (remove when legacy not shipped)
- `src-tauri/binaries/*` — bundled ffmpeg (remove from repo when legacy no longer supported)

Note: keep `ShellFFmpegProcessor` implementation around behind `legacy-ffmpeg` (or simply not compiled when `safe-ffmpeg` is the only engine) to facilitate quick re-enable if needed during early adoption.

#### Addendum: Audit-Driven Extensions and Adjustments

For a consolidated plan addressing audit issues (e.g., build noise, testing gaps, path validation), integrated into P0-P3 phases, see [migration_audit_adjustments.md](../reports/migration_audit_adjustments.md). Key insertions:

- **P0 Insertions:** After task 4: Full path validation (C2.1-C2.3). After task 5: Platform docs (M1.1).
- **P1 Insertions:** Before task 1: Build/Clippy fixes (C1.1-C1.3). After task 4: Tests/cancellation (H1.1-H1.3, H3.1).
- **P2 Insertions:** Before task 1: Refactor oversized code (H2.1-H2.2). After task 3: Medium fixes (M2.1-M2.2, M3.1, M4.1).
- **New P3:** Post-migration cleanup (L1.1, L2.1, L3.1-L3.3); updated testing/CI.

Proceed with original tasks, incorporating these in order for comprehensive coverage.

#### Testing Plan
- Unit:
  - Feature-on tests for packet decode/encode and duration accounting; path validation remains in the `audio` layer.
- Integration (feature-on):
  - Merge 2–3 small files to m4b; assert file exists and `writer` can update tags.
  - Cancellation during encode should abort and leave no zombie processes (trivially satisfied without child process).
- CI gating:
  - Default CI remains legacy path (no extra build deps). Add a separate CI job with `--features safe-ffmpeg` on macOS ARM runners to validate the new engine.

#### Risk & Mitigations
- FFmpeg dev libs availability: Document Homebrew prerequisite and keep feature off by default until validated.
- AAC encoder differences: libavcodec `aac` may not perfectly match `libfdk_aac` quality at same bitrate. We currently re-encode; quality acceptance is a later UX decision.
- Performance: streaming re-encode is CPU-bound; optimize after parity (threaded read/encode, larger packet batches).

#### Rollback Strategy
- Build without `safe-ffmpeg` (default): instantly returns to shell engine.
- Keep a toggle branch `pre_ffmpegnext` until we confirm parity.

#### Acceptance Criteria
- P0 complete when: feature-on build merges inputs to a playable M4B, progress updates, cancel works, tests green, docs updated.
- P1 complete when: feature-on build is selectable as the default engine and repo doesn’t rely on concat files in the new path.
- P2 complete when: legacy shell code is removed from default build and docs/tests reflect the new boundary.

#### Suggestions for proceeding mostly in parallel (atomized tasks):
- Implement core decode→encode in FfmpegNextProcessor (P0).
- Add progress via PTS accumulation and cancellation checks (P0).
- Wire runtime selection with a DefaultProcessor type alias (P1 prep).
- Add a small feature-gated integration test using media/01 - Introduction.mp3 (P0).

You can now enable the feature locally with:
  - `cargo test --features safe-ffmpeg`
  - `cargo clippy --features safe-ffmpeg`
  - `build/run` with the same flag when we start implementing the real pipeline.
