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

**Current Status**: Tests/clippy green in default and --features safe-ffmpeg (with some allows). Large modules remain (e.g., processor.rs at 631 lines). Default engine is shell; feature-on uses ffmpeg-next skeleton.

## Remaining Work by Priority
Tasks are atomized with IDs, step-by-step descriptions, dependencies, verification steps, and effort estimates (Low/Medium/High, considering junior dev guidance [[memory:4478114]]; confirm changes [[memory:4478102]]). This format enhances the original tasks with granularity from the audit.

### P0: Blockers and Security (Focus: Secure FFmpeg, Stabilize Builds)
These must be done first to unblock safe migration and fix risks.

1. **Path Validation and Escaping** (Critical):
   - **Task P0.1.1**: Implement shared input path validation function: Check exists, is regular file, extension in whitelist (mp3/m4a/m4b/etc), canonicalize with Path::canonicalize(), strip invalid chars (CR/LF/NUL). (Deps: None. Verify: New unit tests for edge cases (nonexistent, dir, invalid ext, symlinks). Effort: Low.)
   - **Task P0.1.2**: Apply validation to all input sites (e.g., file_list.rs validate_audio_files, processor.rs process_audiobook_with_context). (Deps: P0.1.1. Verify: Integration test with invalid inputs errors correctly. Effort: Medium.)
   - **Task P0.1.3**: Add symlink policy: Resolve with canonicalize; log warnings for symlinks but proceed. Add output dir write-permission probe (create/test-remove temp file). (Deps: P0.1.1. Verify: Test with symlinked input and read-only output dir. Effort: Low.)

2. **Build/Clippy Noise Reduction** (Critical; Maps to P0):
   - **Task P0.2.1**: Apply consistent gating to ProcessGuard, ProgressContext*, ProcessingContextBuilder using #[cfg(any(test, feature = "safe-ffmpeg"))] on types, impls, and re-exports in audio/mod.rs. (Deps: None. Verify: cargo clippy -- -D warnings and --features safe-ffmpeg both pass. Effort: Medium.)
   - **Task P0.2.2**: Add #[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(dead_code)] to unused helpers (e.g., CleanupGuard methods) temporarily. (Deps: P0.2.1. Verify: No dead_code warnings in default build. Effort: Low.)
   - **Task P0.2.3**: Add CI jobs for default and feature-on Clippy; fail on warnings. (Deps: P0.2.1. Verify: CI passes both matrices. Effort: Low.)

3. **FFmpeg-Next Core Pipeline** (Critical; Maps to P0):
   - **Task P0.3.1**: Flesh out FfmpegNextProcessor::execute: Open each input, find audio stream, decode packets, resample, encode to AAC, mux to m4b (sequential append). (Deps: None. Verify: Small merge test in ffmpegnext_tests.rs with repo media produces playable file. Effort: High.)
   - **Task P0.3.2**: Handle settings: bitrate, channels (mono/stereo), sample rate (explicit or auto from first input). (Deps: P0.3.1. Verify: Test with explicit/auto settings matches output properties. Effort: Medium.)

4. **Progress and Cancellation in FFmpeg-Next** (Critical; Maps to P0):
   - **Task P0.4.1**: Compute progress from accumulated PTS/encoded samples vs total_duration; emit every ~200ms. (Deps: P0.3.1. Verify: Test validates emitted percentages (e.g., 50% at half). Effort: Medium.)
   - **Task P0.4.2**: Poll ctx.is_cancelled() per packet/frame; abort cleanly (flush encoder, delete partial output). (Deps: P0.4.1. Verify: Mid-process cancel test aborts without artifacts. Effort: Low.)

### P1: High-Impact Maintainability (Focus: Refactor Sizes, Test Extraction, Default Flip)
After P0, these improve code quality and enable defaulting to safe engine.

1. **Remaining Test Extraction** (High):
   - **Task P1.1.1**: Extract inline tests from audio/file_list.rs, audio/settings.rs, audio/progress.rs, ffmpeg/command.rs using pub(crate) or adapters. (Deps: None. Verify: Tests run/pass in unit/; no inline tests remain. Effort: Medium.)
   - **Task P1.1.2**: Split integration tests to tests/integration/*.rs; replace remaining unwrap with expect; apply clippy fixes. (Deps: P1.1.1. Verify: cargo test passes; Clippy clean. Effort: Low.)

2. **Module/Function Trimming** (High):
   - **Task P1.2.1**: Split audio/processor.rs into processor/{prepare.rs (validation/workspace), execute.rs (merge), finalize.rs}; update imports (<400 lines/module, <60 lines/function). (Deps: None. Verify: size_budget.sh shows compliance; Clippy passes. Effort: High.)
   - **Task P1.2.2**: Split audio/progress.rs into progress/{reporter.rs, parser.rs, mod.rs}. (Deps: P1.2.1. Verify: Same. Effort: Medium.)
   - **Task P1.2.3**: Split audio/cleanup.rs into cleanup/{guard.rs, ops.rs, mod.rs}. (Deps: P1.2.1. Verify: Same. Effort: Medium.)
   - **Task P1.2.4**: Split commands/mod.rs into commands/{audio.rs, metadata.rs, system.rs, mod.rs}. (Deps: P1.2.1. Verify: Same. Effort: Low.)
   - **Task P1.2.5**: For TS: Split ui/fileList.ts, statusPanel.ts, outputPanel.ts into sub-files (state/dom/actions). (Deps: None. Verify: npm run build succeeds; manual UI test. Effort: Medium.)

3. **Default Engine Flip** (High):
   - **Task P1.3.1**: Use type alias DefaultProcessor = FfmpegNextProcessor when safe-ffmpeg enabled (Shell otherwise). (Deps: P0 complete. Verify: Build with/without feature uses correct processor. Effort: Low.)
   - **Task P1.3.2**: Stop creating concat files in new engine; use plan.input_file_paths directly. (Deps: P1.3.1. Verify: Test merge without concat file. Effort: Low.)
   - **Task P1.3.3**: Deprecate legacy helpers (ffmpeg/*, progress_monitor.rs, etc.) under cfg(not(safe-ffmpeg)). (Deps: P1.3.1. Verify: Deprecation warnings in legacy build. Effort: Low.)

4. **Process Reliability** (High):
   - **Task P1.4.1**: Add progressive shutdown (TERM then KILL) for legacy shell child. (Deps: None. Verify: Cancel test shows clean shutdown. Effort: Low.)
   - **Task P1.4.2**: Finalize RAII guards across session/cleanup. (Deps: P1.4.1. Verify: No leaks in tests. Effort: Medium.)

5. **Tests and Docs** (High):
   - **Task P1.5.1**: Add integration tests for feature-on: Merge 2-3 files, assert playable m4b, test cancel. (Deps: P0.3.1. Verify: Tests pass. Effort: Medium.)
   - **Task P1.5.2**: Update docs: Build notes for Homebrew deps; remove concat/escape refs for new engine. (Deps: None. Verify: Docs accurate. Effort: Low.)

### P2: Optimizations and Cleanup (Focus: Remove Legacy, Polish)
After P1 stability, clean up and optimize. Incorporates Medium issues.

1. **Legacy Removal** (Medium; Maps to P2):
   - **Task P2.1.1**: Delete/gate behind legacy-ffmpeg: ffmpeg/*, progress_monitor.rs, media_pipeline legacy funcs, concat creation. (Deps: P1.3 complete. Verify: Default build has no legacy code. Effort: Medium.)
   - **Task P2.1.2**: Remove bundle.externalBin and binaries/* from default builds. (Deps: P2.1.1. Verify: tauri build succeeds without external bin. Effort: Low.)
   - **Task P2.1.3**: Delete deprecated adapters post-validation. (Deps: P2.1.1. Verify: No deprecated warnings. Effort: Low.)

2. **Refactor Oversized Code in FFmpeg-Next** (High; Maps to P2):
   - **Task P2.2.1**: Split FfmpegNextProcessor execute into 4 helpers (e.g., setup_encoder lines 139-198, process_input 199-293) to <60 lines each; remove temporary too_many_lines allow. (Deps: P0.3.1. Verify: Clippy passes; size_budget.sh. Effort: Medium.)

3. **Performance and UX** (Medium; Maps to P2):
   - **Task P2.3.1**: Parallelize file analysis in file_list.rs with Tokio. (Deps: None. Verify: Faster analysis in tests. Effort: Medium.)
   - **Task P2.3.2**: Optimize ffmpeg-next: Threaded read/encode, larger batches. (Deps: P0.3.1. Verify: Performance benchmarks. Effort: High.)
   - **Task P2.3.3**: Add optional checksum for any remaining bundled binaries. (Deps: None. Verify: Checksum test. Effort: Low.)
   - **Task P2.3.4**: Replace unwrap_or(0.0) in processor.rs total_duration with proper Err if any duration None. (Deps: None. Verify: New test for missing duration. Effort: Low.)
   - **Task P2.3.5**: Audit/convert other unwraps (e.g., canonicalize().unwrap_or_else) to Results. (Deps: P2.3.4. Verify: Grep shows no unwraps in prod. Effort: Medium.)
   - **Task P2.3.6**: Pre-compute per-input durations accurately for progress; adjust if sample rates differ. (Deps: P0.4.1. Verify: Test with mismatched inputs. Effort: Medium.)
   - **Task P2.3.7**: Add test comparing shell vs. ffmpeg-next output (bitrate/quality check). (Deps: P0.3.1. Verify: Test passes with acceptable delta. Effort: Medium.)

4. **Final Cleanup** (Medium; Maps to P2):
   - **Task P2.4.1**: Remove all cfg_attr allow(dead_code) outside tests. (Deps: P0.2 complete. Verify: Clippy passes. Effort: Low.)
   - **Task P2.4.2**: Add CI guard against allow(dead_code) in prod code. (Deps: P2.4.1. Verify: CI fails on added allow. Effort: Low.)
   - **Task P2.4.3**: Consolidate tests: Remove legacy-only; expand ffmpeg-next coverage. (Deps: P2.1.1. Verify: cargo test passes. Effort: Medium.)

### P3: Post-Migration Cleanup and Polish (Focus: Full Cleanup, DRY Fixes)
New phase from audit; after P2.

1. **Deprecated Adapters Removal** (Low; Maps to P3):
   - **Task P3.1.1**: Remove deprecated adapters (e.g., process_audiobook_with_events) and update call sites. (Deps: P2.1.3. Verify: Grep shows no deprecated. Effort: Low.)

2. **TODO Items Resolution** (Low; Maps to P3):
   - **Task P3.2.1**: Integrate session management fully; remove allow(dead_code). (Deps: None. Verify: Grep shows no TODO/FIXME. Effort: Low.)

3. **DRY Violations Fixes** (Low; Maps to P3):
   - **Task P3.3.1**: Extract context creation to legacy_to_context helper; use in all adapters. (Deps: None. Verify: Code search shows no duplication. Effort: Low.)
   - **Task P3.3.2**: Centralize progress emission in a ProgressEmitter trait impl for both processors. (Deps: P3.3.1. Verify: Same. Effort: Low.)
   - **Task P3.3.3**: Derive command preview from Command::get_args() instead of manual format. (Deps: None. Verify: Preview matches executed command. Effort: Low.)

## Updated Testing Plan
- Unit/Integration: Extend with all new tests from tasks (e.g., H1.*, M4.1, path validation).
- CI: Jobs for default and feature-on Clippy; fail on warnings/dead_code outside tests. Add performance benchmarks for ffmpeg-next.

## Updated Risk & Mitigations
- Build noise: Mitigated by P0.2 tasks (gating/allows).
- Testing gaps: By P1.5 and P3 expansions.
- Encoder differences: By P2.3.7 comparison test.
- General: Keep pre_ffmpegnext branch for rollback; confirm all changes [[memory:4478102]].

## Workflow Guidelines
- **Order**: Complete all P0 before P1; one task at a time. Phases imply sequencing (complete P0 before P1), but individual subtasks only depend on explicitly listed "Deps:"—"None" allows independent work within the phase.
- **Per Task**: Plan in this doc, assign to agent, implement, run cargo test + clippy, confirm changes, update status with [X].
- **Priorities**: Tag new items with P0/P1/P2; defer non-essentials.
- **Tracking**: Update this doc as single source; archive old ones.

Last Updated: [DATE]
