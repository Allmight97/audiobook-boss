# Consolidated Roadmap: Refactoring, FFmpeg Migration, and Maintenance

This document summarizes completed work, then breaks remaining tasks into phases prioritized by P0 (blockers/security), P1 (high-impact maintainability), and P2 (optimizations/cleanup). Each task is atomized for ordered workflow: assign to agents, implement one-by-one, test (cargo test + clippy), and confirm before merging.

## Completed Work Summary
- **Test Hygiene**: Inline tests extracted to `src-tauri/tests/unit/**` for errors, audio/session/metrics, metadata/reader/writer, ffmpeg/mod, and commands (split by domain). unwrap/expect fixes and clippy suggestions applied. Processor tests moved to unit/audio/.
- **Tooling**: Added size_budget.sh for reporting large modules/functions. FFmpeg diagnostics (path logging, command preview).
- **Planning Docs**: Created mvp-roadmap.md, refactor-plan.md; initial ffmpeg-next-migration.md.
- **Processing Boundary**: Introduced MediaProcessor trait and ShellFFmpegProcessor; routed execution via trait. Added wait() after cancel for child reaping.
- **Packaging**: Configured bundle.externalBin for ffmpeg-universal; locate_ffmpeg prefers bundled. Dev symlink to Homebrew.
- **Progress UX**: Emitted final stages (Finalizing, Writing Metadata, Completed); no stall at 79%.
- **FFmpeg-Next Bootstrap**: Added safe-ffmpeg feature; initial FfmpegNextProcessor impl (decode/resample/encode/mux); feature-gated tests in unit/audio/ffmpegnext_tests.rs.
- **P1 Noise Reduction (Partial)**: Gated some scaffolding (ProcessingContextBuilder, etc.); temporary allows for dead_code and too_many_lines in feature-on builds.
- **Path validation and Escaping**: Implemented comprehensive path validation system with shared validation function, integrated across all input entry points (file_list, processor, commands), and added symlink resolution with warning logging plus output directory write permission probing. This critical security enhancement prevents path traversal attacks, injection vulnerabilities, and ensures consistent validation behavior throughout the audio processing pipeline. All 63 tests pass including 18 new path validation tests covering edge cases and integration scenarios.
- **Build/Clippy Noise Reduction**: Completed P0.2 Build/Clippy Noise Reduction with proper cfg gating, strategic allows using cfg_attr patterns, and comprehensive CI matrix testing. Fixed all clippy errors (Default trait implementations, format string inlining, too_many_lines allows) ensuring both default and safe-ffmpeg configurations pass cleanly with -D warnings.
- **FFmpeg-Next Core Pipeline (P0.3.1-P0.3.2)**: Completed full implementation of FfmpegNextProcessor with multi-input decode/resample/encode/mux pipeline, proper settings handling (bitrate, channels, sample rate), and comprehensive test coverage. Refactored 200+ line execute function into 12 helper functions (<60 lines each) added 19 new tests covering integration and settings validation. All functionality verified with zero clippy warnings in both default and safe-ffmpeg configurations.
- **Progress & Cancellation (P0.4.1–P0.4.2)**: Added ffmpeg-next progress based on accumulated PTS with ~200ms emits, plus per-frame `ctx.is_cancelled()` polling. Integrated `CleanupGuard` to remove partial outputs on cancel/error, with a best-effort encoder flush; added unit tests for progress math and cancellation cleanup; clippy clean.

### P0: Blockers and Security (Focus: Secure FFmpeg, Stabilize Builds)
These must be done first to unblock safe migration and fix risks.

1.  **Path Validation and Escaping** (Critical): DONE ✅
2.  **Build/Clippy Noise Reduction** (Critical): DONE ✅
3.  **FFmpeg-Next Core Pipeline** (Critical): DONE ✅
4.  **Progress and Cancellation in FFmpeg-Next** (Critical): DONE ✅

### P1: High-Impact Maintainability (Focus: Refactor Sizes, Test Extraction, Default Flip)
After P0, these improve code quality and enable defaulting to safe engine.

1.  **Module/Function Trimming (High Priority)**:
    - **Task P1.1.1**: Split `audio/processor.rs` into `processor/{prepare.rs, execute.rs, finalize.rs}`. Goal: <400 lines/module, <60 lines/function. (Deps: None. Verify: `size_budget.sh` compliance; `cargo clippy` passes. Effort: High.) ✅ **COMPLETED**
    - **Task P1.1.2**: Refactor `media_pipeline.rs` functions with excessive parameters (`process_input_packets`, `process_decoded_frames`, `process_input_file`) to use a context struct. This is a critical step to improve readability and maintainability. (Deps: None. Verify: Reduced parameter count, clean `clippy` run. Effort: Medium.) ✅ **COMPLETED** — See `docs/planning/archive_completed/p1.1.2_media_pipeline_context_refactor_plan.md` for outcomes (context wiring, parameter reduction, validation results).
      - TODO: a minor follow-up may move `stream_index`/`file_index` into context to remove the remaining allow on `process_input_packets`.
    - **Task P1.1.3**: Split `audio/progress.rs` into `progress/{reporter.rs, parser.rs, mod.rs}`. (Deps: P1.1.1. Verify: `size_budget.sh` compliance. Effort: Medium.) ✅ **COMPLETED**
    - **Task P1.1.4**: Split `audio/cleanup.rs` into `cleanup/{guard.rs, ops.rs, mod.rs}`. (Deps: P1.1.1. Verify: `size_budget.sh` compliance. Effort: Medium.)
    - **Task P1.1.5**: Split `commands/mod.rs` into `commands/{audio.rs, metadata.rs, system.rs, mod.rs}`. (Deps: P1.1.1. Verify: `size_budget.sh` compliance. Effort: Low.)
    - **Task P1.1.6**: For TS: Split `ui/fileList.ts` and `ui/statusPanel.ts` into sub-files (e.g., state/dom/actions). (Deps: None. Verify: `npm run build` succeeds; manual UI test. Effort: Medium.)

2.  **Remaining Test Extraction (Medium Priority)**:
    - **Task P1.2.1**: Extract inline tests from `audio/file_list.rs`, `audio/settings.rs`, `audio/progress.rs`, `ffmpeg/command.rs` using `pub(crate)` or adapters. (Deps: None. Verify: Tests run/pass in `unit/`; no inline tests remain. Effort: Medium.)
    - **Task P1.2.2**: Split integration tests to `tests/integration/*.rs`; replace remaining `unwrap` with `expect`; apply clippy fixes. (Deps: P1.2.1. Verify: `cargo test` passes; `Clippy` clean. Effort: Low.)

3.  **Default Engine Flip (Low Priority - Post-Refactor)**:
    - **Task P1.3.1**: Use type alias `DefaultProcessor = FfmpegNextProcessor` when `safe-ffmpeg` enabled (Shell otherwise). (Deps: P1.1 complete. Verify: Build with/without feature uses correct processor. Effort: Low.)
    - **Task P1.3.2**: Stop creating concat files in new engine; use `plan.input_file_paths` directly. (Deps: P1.3.1. Verify: Test merge without concat file. Effort: Low.)
    - **Task P1.3.3**: Deprecate legacy helpers (`ffmpeg/*`, `progress_monitor.rs`, etc.) under `cfg(not(safe-ffmpeg))`. (Deps: P1.3.1. Verify: Deprecation warnings in legacy build. Effort: Low.)

4.  **Process Reliability (Medium Priority)**:
    - **Task P1.4.1**: Add progressive shutdown (TERM then KILL) for legacy shell child. (Deps: None. Verify: Cancel test shows clean shutdown. Effort: Low.)
    - **Task P1.4.2**: Finalize RAII guards across session/cleanup. (Deps: P1.4.1. Verify: No leaks in tests. Effort: Medium.)

5.  **Tests and Docs (Low Priority)**:
    - **Task P1.5.1**: Add integration tests for feature-on: Merge 2-3 files, assert playable m4b, test cancel. (Deps: P0.3.1. Verify: Tests pass. Effort: Medium.)
    - **Task P1.5.2**: Update docs: Build notes for Homebrew deps; remove concat/escape refs for new engine. (Deps: None. Verify: Docs accurate. Effort: Low.)

### P2: Optimizations and Cleanup (Focus: Remove Legacy, Polish)
After P1 stability, clean up and optimize.

1.  **Legacy Removal** (Medium):
    - **Task P2.1.1**: Delete/gate behind `legacy-ffmpeg`: `ffmpeg/*`, `progress_monitor.rs`, media_pipeline legacy funcs, concat creation. (Deps: P1.3 complete. Verify: Default build has no legacy code. Effort: Medium.)
    - **Task P2.1.2**: Remove `bundle.externalBin` and `binaries/*` from default builds. (Deps: P2.1.1. Verify: `tauri build` succeeds without external bin. Effort: Low.)
    - **Task P2.1.3**: Delete deprecated adapters post-validation. (Deps: P2.1.1. Verify: No deprecated warnings. Effort: Low.)

2.  **Refactor Oversized Code in FFmpeg-Next** (High):
    - **Task P2.2.1**: Split `FfmpegNextProcessor execute` into helpers (e.g., `setup_encoder`, `process_input`) to <60 lines each; remove temporary `too_many_lines` allow. (Deps: P1.1.2. Verify: Clippy passes; `size_budget.sh`. Effort: Medium.)

3.  **Performance and UX** (Medium):
    - **Task P2.3.1**: Parallelize file analysis in `file_list.rs` with Tokio. (Deps: None. Verify: Faster analysis in tests. Effort: Medium.)
    - **Task P2.3.2**: Optimize `ffmpeg-next`: Threaded read/encode, larger batches. (Deps: P0.3.1. Verify: Performance benchmarks. Effort: High.)
    - **Task P2.3.3**: Add optional checksum for any remaining bundled binaries. (Deps: None. Verify: Checksum test. Effort: Low.)
    - **Task P2.3.4**: Replace `unwrap_or(0.0)` in `processor.rs` `total_duration` with proper `Err` if any duration `None`. (Deps: None. Verify: New test for missing duration. Effort: Low.)
    - **Task P2.3.5**: Audit/convert other `unwraps` (e.g., `canonicalize().unwrap_or_else`) to Results. (Deps: P2.3.4. Verify: Grep shows no unwraps in prod. Effort: Medium.)
    - **Task P2.3.6**: Pre-compute per-input durations accurately for progress; adjust if sample rates differ. (Deps: P0.4.1. Verify: Test with mismatched inputs. Effort: Medium.)
    - **Task P2.3.7**: Add test comparing shell vs. ffmpeg-next output (bitrate/quality check). (Deps: P0.3.1. Verify: Test passes with acceptable delta. Effort: Medium.)

4.  **Final Cleanup** (Medium):
    - **Task P2.4.1**: Remove all `cfg_attr allow(dead_code)` outside tests. (Deps: P0.2 complete. Verify: Clippy passes. Effort: Low.)
    - **Task P2.4.2**: Add CI guard against `allow(dead_code)` in prod code. (Deps: P2.4.1. Verify: CI fails on added allow. Effort: Low.)
    - **Task P2.4.3**: Consolidate tests: Remove legacy-only; expand ffmpeg-next coverage. (Deps: P2.1.1. Verify: `cargo test` passes. Effort: Medium.)

### P3: Post-Migration Cleanup and Polish (Focus: Full Cleanup, DRY Fixes)
After P2.

1.  **Deprecated Adapters Removal** (Low):
    - **Task P3.1.1**: Remove deprecated adapters (e.g., `process_audiobook_with_events`) and update call sites. (Deps: P2.1.3. Verify: Grep shows no deprecated. Effort: Low.)

2.  **TODO Items Resolution** (Low):
    - **Task P3.2.1**: Integrate session management fully; remove `allow(dead_code)`. (Deps: None. Verify: Grep shows no TODO/FIXME. Effort: Low.)

3.  **DRY Violations Fixes** (Low):
    - **Task P3.3.1**: Extract context creation to `legacy_to_context` helper; use in all adapters. (Deps: None. Verify: Code search shows no duplication. Effort: Low.)
    - **Task P3.3.2**: Centralize progress emission in a `ProgressEmitter` trait impl for both processors. (Deps: P3.3.1. Verify: Same. Effort: Low.)
    - **Task P3.3.3**: Derive command preview from `Command::get_args()` instead of manual format. (Deps: None. Verify: Preview matches executed command. Effort: Low.)

## Updated Testing Plan
- Unit/Integration: Extend with all new tests from tasks (e.g., P1.*, P2.*, path validation).
- CI: Jobs for default and feature-on Clippy; fail on warnings/dead_code outside tests. Add performance benchmarks for ffmpeg-next.

## Updated Risk & Mitigations
- Build noise: Mitigated by P0.2 tasks (gating/allows).
- Testing gaps: By P1.2 and P2 expansions.
- Encoder differences: By P2.3.7 comparison test.
- General: Keep `pre_ffmpegnext` branch for rollback; confirm all changes [[memory:4478102]].

## Workflow Guidelines
- **Order**: Complete all P1 tasks before P2. The highest priority is **P1.1 Module/Function Trimming**.
- **Per Task**: Plan in this doc, assign to agent, implement, run `cargo test` + `clippy`, confirm changes, update status with [X].
- **Priorities**: Tag new items with P1/P2/P3; defer non-essentials.
- **Tracking**: Update this doc as single source; archive old ones.

Last Updated: 2025-08-11
