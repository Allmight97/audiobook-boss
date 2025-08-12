## L4 Quality Audit and Draft Updated Roadmap (2025-08-12)

This audit reviews the current codebase state against an L4 quality bar and proposes a refreshed, actionable roadmap. It uses `docs/planning/consolidated-roadmap.md` as a reference baseline and integrates open items from `docs/planning/progress_bug_tracker.md`.

### Executive summary
- **Overall**: Architecture, path security, and the processor split are in strong shape. Remaining gaps are mainly around metadata/cover art integration, cancellation UX semantics, and legacy/dead-code cleanup.
- **Blocking gaps (P0)**: End-to-end cover art writing during processing; consistent “cancelled” event semantics; UI cover art persistence rules while switching selections.
- **Maintainability (P1)**: Remove transitional `allow(dead_code)` and introduce feature gating for legacy adapters; finalize engine flip preparation; unify event and stage naming; tighten ffmpeg-next helpers to eliminate `too_many_arguments` allows.
- **Cleanup/optimizations (P2+)**: Legacy removal after flip; CI guard against dead code allows; performance passes and parity tests.

### Quality rating (1–5)
- **Correctness**: 4 — Strong validation and staged pipeline. Gaps: cover art not written during processing; cancellation UI semantics.
- **Design/Modularity**: 4 — Clear split across `prepare/execute/finalize` and `MediaProcessor`. Some transitional adapters and allows remain.
- **Robustness**: 4 — Centralized path validation and cleanup guards good; need consistent cancelled event emission and progressive shutdown alignment.
- **Tests/Observability**: 4 — Good unit/integration coverage; add focused tests for cover art write path and cancel semantics.
- **Dev-Experience**: 4 — Readable modules, explicit stages; reduce noise from `allow(dead_code)` and TODOs; align doc references.
- **Performance**: 3 — Baseline OK; performance tasks deferred; retain P2 items for ffmpeg-next throughput.
- **Security**: 5 — Path validation and escaping are thorough; no prod `unwrap()` usage outside tests.

To reach Level 5, prioritize: (1) end-to-end cover art integration and tests, (2) consistent cancel event semantics and UI behavior, (3) remove transitional allows with CI guard, (4) finalize engine flip and legacy gating.

### Out-of-date items in consolidated roadmap (observed)
- Reference should point to archived combined plan: `docs/planning/archive_completed/p1.1.4-1.1.6_combined_split_plan.md` (current text points to the non-archived path).
- “Module/Function Trimming (High Priority)” appears completed; leave as DONE with the archived reference above.
- Keep P1.3 Default Engine Flip as not done; no `type DefaultProcessor = ...` alias exists yet (selection happens via `#[cfg(feature = "safe-ffmpeg")]`).

### Key findings and evidence
- **Cover art not written during processing**: finalize stage calls `write_metadata` but not `write_cover_art`, and UI does not pass cover art into processing metadata.

```106:161:src-tauri/src/audio/processor/finalize.rs
/// Writes metadata if provided (UI emission included)
pub(crate) fn write_metadata_stage(
    context: &ProcessingContext,
    merged_output: &PathBuf,
    metadata: Option<AudiobookMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<()> {
    if let Some(metadata) = metadata {
        let ui = crate::audio::progress::ProgressEmitter::new(context.window.clone());
        ui.emit_metadata_start("Writing metadata...");
        reporter.set_stage(ProcessingStage::WritingMetadata);
        write_metadata(merged_output, &metadata)?;
        if context.is_cancelled() {
            return Err(AppError::InvalidInput(
                "Processing was cancelled".to_string(),
            ));
        }
    }
    Ok(())
}
```

```70:102:src-tauri/src/metadata/writer.rs
/// Writes cover art to an M4B file
pub fn write_cover_art<P: AsRef<Path>>(
    file_path: P,
    cover_data: &[u8],
) -> Result<()>
    let path = file_path.as_ref();
    ...
    tag.push_picture(picture);
    tagged_file.save_to_path(path, Default::default())?;
    Ok(())
}
```

- **UI cancel semantics**: UI sets local status to `cancelled` on button press without a backend event; backend sets a flag and attempts to kill, but no explicit `cancelled` event is emitted.

```221:233:src/ui/statusPanel/logic.ts
private async handleCancel(): Promise<void> {
  await invoke('cancel_processing');
  this.updateStatus({ stage: 'cancelled', percentage: this.currentStatus.percentage, message: 'Cancellation requested...' });
}
```

- **Transitional allows**: Multiple `allow(dead_code)` across audio modules, plus `#[allow(clippy::too_many_arguments)]` in ffmpeg-next internals. Good targets for P1/P2 cleanup.

### Draft updated roadmap (proposed)

#### P0: Blockers and user-facing bugs (stabilize E2E)
1. Cover art end-to-end during processing
   - Backend: In `finalize::write_metadata_stage`, if `metadata.cover_art.is_some()`, call `metadata::writer::write_cover_art` after `write_metadata`.
   - Frontend: Include selected cover art when invoking `process_audiobook_files` (plumb from `coverArt` module into `StatusPanel.getCurrentMetadata()` or pass as a separate field).
   - Tests: Add unit test for `write_cover_art` on a temp m4b and an integration test that processes with cover art, then reads it back.
   - Acceptance: Output file reliably contains the provided cover art; existing metadata preserved as designed.

2. Consistent cancellation semantics and UX
   - Backend: Emit a `processing-progress` event with `stage='cancelled'` when cancellation is observed (both engines). Ensure progressive shutdown path (TERM→KILL) is used or simulated consistently.
   - Frontend: Stop setting stage to `cancelled` optimistically; rely on backend event to transition. Show “Cancellation requested…” interim state.
   - Acceptance: Pressing Cancel stops work and UI returns to idle with a cancelled toast; no zombie ffmpeg processes.

3. UI cover art persistence across file selection
   - Behavior: If the user manually loads cover art, persist it across file selection changes until the user clears or overrides it. Only auto-populate artwork from file metadata when no custom cover art is set.
   - Implementation: Track a `hasCustomCoverArt` flag in the cover art module; gate `populateMetadataForm` from overwriting when true.
   - Acceptance: Custom cover art remains intact when clicking different files; Clear button resets and becomes hidden appropriately.

4. Update roadmap references and docs
   - Fix consolidated roadmap link to the archived combined plan path, and mark the trimming section as fully DONE with the archived reference.

#### P1: Maintainability and default engine preparation
1. Legacy adapter gating + dead code allows removal
   - Introduce a `legacy-adapters` feature (or align with `not(feature = "safe-ffmpeg")`) to hide `audio/processor/legacy.rs` in modern builds.
   - Remove `#![allow(dead_code)]` across `audio/*` where unused code is eliminated or feature-gated. Add a CI lint in P2.
   - Acceptance: Default and `--features safe-ffmpeg` builds are clippy-clean with no dead-code allows in prod code.

2. Event naming unification and stage ranges
   - Drop the unused `merging` stage from the frontend types if no longer emitted; keep `converting` and `writing` only.
   - Centralize message constants to avoid string drift; keep `ProgressEmitter` the single emission surface.

3. Default engine flip prep
   - Optional: Add `type DefaultProcessor = ...` alias to clarify engine selection intent.
   - Confirm shell fallback remains functional; ensure both configurations are covered by CI and docs.

4. ffmpeg-next internals: tighten APIs
   - Reduce `#[allow(clippy::too_many_arguments)]` by moving `stream_index`/`file_index` into the context, and splitting helpers further if needed.

#### P2: Cleanup, performance, parity
1. Legacy removal and packaging simplification
   - After flip validation, delete or feature-gate `ffmpeg/*`, `progress_monitor.rs`, and concat-file creation for ffmpeg-next builds; simplify Tauri packaging to drop external FFmpeg where possible.

2. CI and lint guards
   - Add CI checks to fail on `allow(dead_code)` outside tests and to run clippy with `-D warnings` on both configurations.

3. Performance improvements and parity tests
   - Add performance baselines for multi-file processing; ffmpeg-next threading/batching experiments; parity test comparing shell vs ffmpeg-next bitrate/quality within acceptable delta.

### Bug triage and fix plan (from progress_bug_tracker.md)
- [X] Clear cover art button visibility
  - Current state: `coverArt.setCoverArt()` calls `updateClearButtonVisibility()`. Re-verify; if still repros, ensure `populateMetadataForm` uses `setCoverArt` (it already does) and initial `initCoverArt()` is called at app start.

- [X] Clear file list feature — implemented.

- [X] “Starting FFmpeg merge ... when only 1 file?” — log text originates from execute stage; total duration reflects sum of valid files. If still confusing, adjust message for single-file case (P3 polish).

- [ ] Multiple files as separate jobs — feature request; out of scope for P0/P1; add to backlog after engine flip.

- [ ] Load cover art from URL — feature request; defer until cover art pipeline is complete; can reuse `load_cover_art_file` pattern with HTTP fetch and validation.

- [ ] Cover art overwritten when switching selection — addressed in P0.3 above.

- [ ] Output M4B lacks cover art — addressed in P0.1 above (write during finalize and plumb from UI).

- [ ] Cancel button doesn’t cancel — addressed in P0.2 above (event emission + UX).

### Notes on evidence and security posture
- Path validation is centralized and thorough (canonicalization, extension checks, symlink warnings). No prod `unwrap()` usages surfaced outside test modules.
- ffmpeg-next progress calculation emits at ~200ms intervals and checks cancellation; shell path uses the progress monitor with cancellation kill and reap.

### Next steps
- Implement P0 items and update `docs/planning/consolidated-roadmap.md` accordingly. Run `cargo test`, `cargo test --features safe-ffmpeg`, and `cargo clippy -- -D warnings` on both configurations.
- After P0 merges, proceed with P1 maintainability tasks and CI guard additions.

### L6 overlay note
The staged processor architecture and `MediaProcessor` boundary create a reusable pattern for future format adapters and additional processing stages (e.g., normalization, chapterization). Consolidating progress emission and finalize-side effects behind cohesive modules positions the project for safe deprecation of legacy shell code post-flip without touching UI contracts.


