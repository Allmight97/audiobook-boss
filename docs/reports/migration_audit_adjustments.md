# Consolidated Migration Audit Adjustments Plan

This document synthesizes the atomized implementation plan for addressing audit issues with the addendum to the FFmpeg-Next migration plan. It is organized by issue priority (Critical, High, Medium, Low) with atomized tasks, followed by an integrated phased extension to the migration plan (P0-P3). Tasks are cross-referenced to phases for clarity. This ensures all issues are addressed in coherent order, with no room for interpretation. Reference the original audit for context.

The plan aligns with P0/P1/P2 prioritization [[memory:5496753]], extending to P3 for post-migration items. Tasks include: Task ID, Description (step-by-step), Dependencies, Verification, Effort (Low/Medium/High, per junior dev guidance [[memory:4478114]]; confirm before changes [[memory:4478102]]).

## Atomized Tasks by Priority

### Critical Issues (Fix during P0-P2)
1. **Build Asymmetries and Clippy Noise** (Dead code, gating mismatches, too_many_lines). [Maps to: Extended P1]
   - **Task C1.1**: Apply consistent gating to `ProcessGuard`, `ProgressContext*`, and `ProcessingContextBuilder` using `#[cfg(any(test, feature = "safe-ffmpeg"))]` on types, impls, and re-exports in `audio/mod.rs`. (Deps: None. Verify: `cargo clippy -- -D warnings` and `--features safe-ffmpeg` both pass. Effort: Medium.)
   - **Task C1.2**: Add `#[cfg_attr(not(any(test, feature = "safe-ffmpeg))), allow(dead_code)]` to unused helpers (e.g., `CleanupGuard` methods) as a temporary measure. (Deps: C1.1. Verify: No dead_code warnings in default build. Effort: Low.)
   - **Task C1.3**: Refactor `FfmpegNextProcessor::execute` into 4 helpers: `setup_encoder` (lines 139-198), `process_input` (199-293 per input), `flush_decoder` (294-322), `flush_encoder` (324-335). Each <60 lines. (Deps: C1.1. Verify: Clippy passes without too_many_lines; function sizes via `size_budget.sh`. Effort: Medium.)

2. **Incomplete Path Validation** (Existence, is_file, extension whitelist, canonicalize). [Maps to: Extended P0]
   - **Task C2.1**: In `MediaProcessingPlan::new`, add a loop over `input_file_paths`: Check `path.exists()`, `path.is_file()`, extension in whitelist (mp3/m4a/m4b), and canonicalize with `Path::canonicalize()`. Return Err on failure. (Deps: None. Verify: New unit test for invalid paths errors. Effort: Low.)
   - **Task C2.2**: Add symlink policy: Use `std::fs::canonicalize` to resolve symlinks; log warnings for symlinks but proceed. (Deps: C2.1. Verify: Test with symlinked input file. Effort: Low.)
   - **Task C2.3**: Implement output dir write-permission probe: In `MediaProcessingPlan::new`, create/test-remove a temp file in `output_path.parent()`. (Deps: C2.1. Verify: Test errors on read-only dir. Effort: Low.)

### High Issues (Fix after Migration, in P3)
1. **Testing Gaps** (Multi-input, cancellation, progress accuracy). [Maps to: Extended P1, P3]
   - **Task H1.1**: Add test in `ffmpegnext_tests.rs`: Process 2-3 inputs (duplicate test MP3), assert output duration = sum(inputs), PTS continuous (no gaps). (Deps: None. Verify: Test passes with `cargo test --features safe-ffmpeg`. Effort: Medium.)
   - **Task H1.2**: Add cancellation test: Start processing, set `context.is_cancelled=true` mid-stream, assert clean abort (no partial file, Err returned). (Deps: H1.1. Verify: Test passes. Effort: Medium.)
   - **Task H1.3**: Add progress test: Validate emitted percentages match expected (e.g., 50% at half duration); test clamping (>100%). (Deps: H1.1. Verify: Test passes. Effort: Low.)

2. **Oversized Code** (Modules/functions > limits). [Maps to: Extended P2]
   - **Task H2.1**: Split `processor.rs` (>600 lines) into `processor/prepare.rs` (validation/workspace), `processor/execute.rs` (merge/finalize). Update imports. (Deps: None. Verify: `size_budget.sh` shows <400 lines; Clippy passes. Effort: High.)
   - **Task H2.2**: Refactor other large functions (e.g., `process_audiobook_with_context`) into <60-line segments. (Deps: H2.1. Verify: Same as above. Effort: Medium.)

3. **Cancellation Handling Parity** (Flush/partial cleanup). [Maps to: Extended P1]
   - **Task H3.1**: In `FfmpegNextProcessor::execute`, on cancel: Flush encoder, delete partial output file. (Deps: None. Verify: Cancellation test from H1.2 leaves no artifacts. Effort: Low.)

### Medium Issues (Opportunistic Fixes)
1. **Platform Dependencies** (Dev libs documentation). [Maps to: Extended P0]
   - **Task M1.1**: Add to `docs/development.md`: "For safe-ffmpeg on macOS ARM: `brew install ffmpeg`. For Linux/Windows: Install libav* dev packages (e.g., apt install libavcodec-dev)." (Deps: None. Verify: Doc exists; build tested on another platform. Effort: Low.)

2. **Risky Patterns** (Unwraps/expects). [Maps to: Extended P2]
   - **Task M2.1**: Replace `unwrap_or(0.0)` in `processor.rs::total_duration` with proper error (e.g., if any duration None, Err). (Deps: None. Verify: New test for missing duration errors. Effort: Low.)
   - **Task M2.2**: Audit/convert other unwraps (e.g., `canonicalize().unwrap_or_else`) to Results. (Deps: M2.1. Verify: No unwraps in production via grep. Effort: Medium.)

3. **Progress Inaccuracies** (Mismatched inputs). [Maps to: Extended P2]
   - **Task M3.1**: In `FfmpegNextProcessor`, pre-compute per-input durations accurately for progress; adjust if sample rates differ. (Deps: None. Verify: Test with mismatched inputs shows accurate progress. Effort: Medium.)

4. **Encoder Parity** (Quality/bitrate differences). [Maps to: Extended P2]
   - **Task M4.1**: Add test: Compare shell vs. ffmpeg-next output (e.g., via audio diff tool or bitrate check). Log warnings if quality differs. (Deps: None. Verify: Test passes with acceptable delta. Effort: Medium.)

### Low Issues (Low-Impact Cleanup)
1. **Deprecated Adapters** (Legacy functions). [Maps to: P3]
   - **Task L1.1**: After P2, remove deprecated adapters (e.g., `process_audiobook_with_events`) and update call sites. (Deps: P2 complete. Verify: Grep shows no deprecated usages. Effort: Low.)

2. **TODO Items** (e.g., session.rs dead_code allow). [Maps to: P3]
   - **Task L2.1**: Integrate session management fully; remove allow(dead_code). (Deps: None. Verify: Grep shows no TODO/FIXME. Effort: Low.)

3. **DRY Violations** (Duplicated logic). [Maps to: P3]
   - **Task L3.1**: Extract context creation to `legacy_to_context` helper; use in all adapters. (Deps: None. Verify: Code search shows no duplication. Effort: Low.)
   - **Task L3.2**: Centralize progress emission in a `ProgressEmitter` trait impl for both processors. (Deps: L3.1. Verify: Same. Effort: Low.)
   - **Task L3.3**: Derive command preview from `Command::get_args()` instead of manual format. (Deps: None. Verify: Preview matches executed command. Effort: Low.)

## Integrated Migration Plan Extensions (P0-P3)
This section extends the core plan in `ffmpeg-next-migration.md` with the above tasks slotted in. Reference the original for base tasks; these are insertions.

### Extended P0 (Enable Parallel Path; No Default Switch)
- Insertion after original task 4 (Tests and lint): Implement full path validation (Tasks C2.1-C2.3).
- Insertion after original task 5 (Docs): Document platform dev libs (Task M1.1).

### Extended P1 (Flip-by-Project-Level Setting; Start Retiring Legacy)
- Insertion before original task 1 (Feature flag scaffold): Fix build/Clippy issues (Tasks C1.1-C1.3).
- Insertion after original task 4 (Progress parity & UI UX): Add high-priority tests and cancellation parity (Tasks H1.1-H1.3, H3.1).

### Extended P2 (Cleanup and Removal Once Stable)
- Insertion before original task 1 (Remove legacy shell code): Refactor oversized code (Tasks H2.1-H2.2).
- Insertion after original task 3 (Expand tests): Address medium items (Tasks M2.1-M2.2, M3.1, M4.1).

### New P3 (Post-Migration Cleanup and Polish)
- Remove deprecated adapters (Task L1.1).
- Resolve TODO items (Task L2.1).
- Fix DRY violations (Tasks L3.1-L3.3).
- Final verification: Run `cargo test --features safe-ffmpeg`, `cargo clippy --features safe-ffmpeg -- -D warnings`, and `size_budget.sh`; all green, no warnings/deprecated/TODO.

### Updated Testing Plan
- Extend unit/integration with all new tests from tasks (H1.*, M4.1, etc.).
- CI: Add jobs for default and feature-on Clippy; fail on warnings/dead_code outside tests.

### Updated Risk & Mitigations
- Build noise mitigated by Extended P1 fixes; testing gaps by Extended P1/P2 expansions.

This consolidated plan addresses all audit issues in priority order, fully integrated with the migration phases.