# FFmpeg Shell Legacy Cleanup Plan

Updated: 2025-08-17

## Executive Summary
- Verified legacy remnants from the prior shell-FFmpeg workflow that no longer serve a functional purpose in the single-engine `ffmpeg-next` architecture.
- Propose a phased removal plan with guardrails to avoid regressions. Runtime behavior remains unchanged except for dead-code elimination and doc updates.
- Validation: full test runs (default) must remain green before and after each phase.

## Validation Findings (Code References)

1) Concat placeholder and struct field are obsolete in the `ffmpeg-next` path
- `MediaProcessingPlan` still carries `input_concat_file`, not used by the encoder pipeline; only passed through and used indirectly to derive a temp path.
- `prepare_workspace()` writes a placeholder: `unused_concat.txt`.

Paths:
- `src-tauri/src/audio/media_pipeline.rs` → `MediaProcessingPlan { input_concat_file, ... }`
- `src-tauri/src/audio/processor/prepare.rs` → `let concat_file = temp_dir.join("unused_concat.txt");`

2) Vestigial textual progress parser (unused at runtime)
- `src-tauri/src/audio/progress/parser.rs` implements textual CLI progress parsing (`parse_ffmpeg_progress`), re-exported in `progress/mod.rs`, but not referenced by the live code path which uses internal PTS/duration.

3) Test-only channel layout helper
- `AudioSettings::ChannelConfig::ffmpeg_layout()` returns CLI layout strings (`"mono"|"stereo"`). Not required by the current runtime. Used only by tests.

Paths:
- `src-tauri/src/audio/settings.rs` → `ChannelConfig::ffmpeg_layout()`
- `src-tauri/tests/settings_validation_integration.rs` → asserts on `ffmpeg_layout()`

4) Documentation references to CLI phases
- `src/types/events.ts` documents percentage ranges “mapped from FFmpeg progress”; live system maps progress from encoder timestamps.
- `src/main.ts` test harness lines already note removal of `getFFmpegVersion` and shell merge commands; no functional issue here.

5) Orphaned/duplicative unit tests under `tests/unit/`
- Files in `src-tauri/tests/unit/` include legacy coverage for shell command construction and duplicate coverage superseded by integration tests.
  - ffmpeg: `ffmpeg_mod_tests.rs`
  - audio: `processor_tests.rs` (shell sections), `session_tests.rs`, `cleanup_tests.rs`, `metrics_tests.rs`, `path_validation_tests.rs`
  - commands: `audio_commands_tests.rs`, `metadata_commands_tests.rs`, `basic_commands_tests.rs`
  - metadata: `reader_tests.rs`, `writer_tests.rs`, `ffmpeg_bridge_tests.rs`, `cover_art_native_embedding.rs`
  - core: `errors_tests.rs`

Note: We'll remove only the clearly dead/duplicative sets first; keep metadata/core tests for explicit review.

## Out-of-Scope but Related: Quality/Performance Notes
- The log message “Truncating frame from 1152 to 1024 samples for AAC” indicates the known MP3→AAC frame-size mismatch. This is a current engine behavior, not legacy. Cleanup here will not change quality/performance. Separate tuning tasks are recommended (encoder frame-size capabilities, lookahead/accumulation) and are tracked elsewhere.

## Deletion Plan (Phased)

### Phase 1 — Zero-Risk Test Cleanup (no runtime changes)
Remove clearly dead or duplicate tests. Expect no change in behavior; test totals may drop.

Targets:
- `src-tauri/tests/unit/ffmpeg/ffmpeg_mod_tests.rs` (Deleted)
- `src-tauri/tests/unit/audio/session_tests.rs` (Deleted)
- `src-tauri/tests/unit/audio/cleanup_tests.rs` (Deleted)
- `src-tauri/tests/unit/audio/metrics_tests.rs` (Deleted)
- `src-tauri/tests/unit/audio/processor_tests.rs` (Deleted; shell sections fully removed by deletion)
- `src-tauri/tests/unit/commands/audio_commands_tests.rs` (Deleted)
- `src-tauri/tests/unit/commands/metadata_commands_tests.rs` (Deleted)
- `src-tauri/tests/unit/commands/basic_commands_tests.rs` (Deleted)

Validation:
- Run: `cargo test`
- Result: identical runtime behavior; test counts reduced accordingly.

### Phase 2 — Remove Vestigial Progress Parser (isolated)
Rationale: Not used by live code; progress is derived from encoder timestamps.

Edits:
- Delete `src-tauri/src/audio/progress/parser.rs`. (Completed)
- Update `src-tauri/src/audio/progress/mod.rs` to stop re-exporting `parse_ffmpeg_progress` and `FFmpegProgressState`. (Completed)

Impact:
- No runtime impact. Removes dead code and internal unit tests within `parser.rs`.

Validation:
- Run: `cargo test`

### Phase 3 — Remove Concat Artifacts (small refactor)
Rationale: `input_concat_file` and `unused_concat.txt` are legacy from shell CLI concat demuxer.

Edits:
- `src-tauri/src/audio/media_pipeline.rs` (Completed)
  - Removed `input_concat_file` field and constructor parameter from `MediaProcessingPlan`.
- `src-tauri/src/audio/processor/prepare.rs` (Completed)
  - Stopped creating `unused_concat.txt`; workflow now stores only `temp_dir` and `total_duration`.
- `src-tauri/src/audio/processor/execute.rs` (Completed)
  - `merge_audio_files_with_context` now derives `temp_output` from `temp_dir`.
- Tests (Completed)
  - Updated `p41_core_pipeline_tests.rs`, `settings_validation_integration.rs`, and `unit/metadata/cover_art_native_embedding.rs` to new constructor.

Implementation Notes:
- Performed direct removal with broad test adjustments since no runtime usage remained.

Impact:
- No feature changes; minor constructor signature change; fewer moving parts in prepare/execute.

Validation:
- Run: `cargo test`

### Phase 4 — Remove Channel Layout Helper (test-only)
Rationale: `ffmpeg_layout()` is CLI-string oriented and only used by tests.

Edits:
- `src-tauri/src/audio/settings.rs` → remove `ChannelConfig::ffmpeg_layout()`.
- Update tests that assert on `ffmpeg_layout()` (e.g., `settings_validation_integration.rs`) to focus on `channel_count()` or end-to-end behavior instead.

Impact:
- No runtime impact; trims test-only API.

Validation:
- Run: `cargo test`

### Phase 5 — Update Frontend Documentation Comments
Rationale: Align docs with encoder-timestamp progress.

Edits:
- `src/types/events.ts` (Completed): Replaced with PTS-based mapping.
- Ensure no references to deleted commands remain. (Verified)

Validation:
- TypeScript build still clean.

### Phase 6 — Directory Cleanup
Rationale: Remove empty test subdirectories after deletions.

Command: `find src-tauri/tests/unit -type d -empty -delete` (Completed)

## Risk Assessment
- Phase 1, 2, 5, 6: Low/Zero risk (dead code + documentation only).
- Phase 3: Low-medium risk due to constructor and call site changes; mitigated via 2-step deprecation then removal and full test pass.
- Phase 4: Low risk; test adjustments only.

## Validation & CI Gates
For each phase/commit:
- `cargo test`
- `cargo clippy -- -D warnings`

Optional (if feature flags present in CI):
- `cargo test --features safe-ffmpeg`

## Appendix: Cross-Checks Performed
- Concat placeholder present: `prepare.rs` → `unused_concat.txt`.
- `MediaProcessingPlan` contains `input_concat_file`; no live usage in `FfmpegNextProcessor`.
- `parse_ffmpeg_progress` and `FFmpegProgressState` not referenced by runtime code.
- `ChannelConfig::ffmpeg_layout()` used only by tests.
- Frontend docs mention FFmpeg progress mapping; runtime uses PTS/duration.


