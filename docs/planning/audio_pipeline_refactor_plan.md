# Audio Pipeline Refactor & Quality/Performance Improvement Plan (P12)

Scope: Fix 11% sample loss, speed up processing, dismantle `media_pipeline.rs` (878 LOC) into cohesive units, and introduce optional higher‑quality AAC (twoloop + VBR) with UI controls.

## Objectives (Order Matters)
1. Preserve 100% audio samples (eliminate truncation 1152→1024 loss).
2. Improve throughput (target: ≥2× current MB/s baseline in Release build).
3. Reduce `media_pipeline.rs` to façade ≤150 LOC via modular split.
4. Add AAC encoder enhancements (twoloop, threading, optional VBR quality mode 48–68 kbps speech target).
5. Expose CBR/VBR mode + bitrate OR quality UI controls.
6. Add tests & metrics ensuring no regression (duration, sample count, performance sanity).

## Phase 0 – Baseline & Guardrails (Short)
Tasks:
- Run current processing in Release (`cargo run --release` via Tauri) on a fixed 5‑file sample set; capture: elapsed seconds, output duration, throughput, log sample of truncation messages.
- Record: input total samples (estimate = frames * 1152) vs output duration * sample_rate * channels.
Artifacts:
- `docs/reports/audio_baseline_metrics.md` (auto-created in phase output) – simple table.
Acceptance:
- Baseline doc committed.

## Phase 1 – Audio Integrity (Accumulator) & Logging Throttle
Tasks:
- Add `src-tauri/src/audio/buffer.rs` with `SampleAccumulator` (planar f32 per channel, push(), drain_full_frames(frame_size), flush_tail(pad=true)).
- Modify `process_decoded_frames`:
  - Remove truncation branch.
  - Feed accumulator; encode only full 1024-sample frames.
  - On decoder EOF: flush_tail (pad with zeros to 1024) unless exact fit.
- Remove unused giant `flush_decoder_frames` (delete or shrink to no-op) once verified unnecessary.
- Add sample counters to context: `input_samples_total`, `encoded_samples_total`.
- Emit summary log: `AudioSampleStats input=.. encoded=.. loss_percent=..`.
- Downgrade per-packet INFO to DEBUG; keep periodic (every 1000 packets) INFO.
Acceptance:
- No "Truncating frame" logs.
- Loss percent ≤0.02% (tail padding only) or exactly 0 if padded.
- Unit test: accumulator splits 2304 samples into 2 frames.

### Regression Freeze Gate (end of Phase 1)
Once Phase 1 acceptance criteria are satisfied and a manual spot listen passes, core audio processing enters a Regression Freeze:
1. Any change causing failure to emit an M4B, encoder send errors, NaN/Inf sample warnings, or >0.02% sample loss is a P0 blocker and must be fixed before further feature work merges.
2. Accumulator / frame sizing logic is stability-critical; optimization PRs must add/extend tests asserting: output file exists, duration within ±0.5s of summed inputs, and no ERROR logs containing "Encoder send failed" or "NaN".
3. Add Phase 1.5 smoke test: process a 3‑file fixture; assert output presence, duration tolerance, and scan logs (captured buffer) for disallowed strings. This test runs in CI.
4. Experimental processing changes must occur on `exp/` branches; merging to main/design requires green smoke + integration tests.
5. Freeze lifts only after Phase 3 module split completes and new baseline metrics recorded.

Action Item: Implement Phase 1.5 smoke test immediately after current regression (variable frame_size=0 handling) fix is validated.

## Phase 2 – Encoder Enhancements (Threading + Twoloop + VBR Plumbing)
Tasks:
- Threading: set encoder thread count (frame threading) if supported; fallback silently.
- Twoloop: keep existing attempt; add metrics log `aac_twoloop=enabled|fallback`.
- VBR option plumbing (backend only now):
  - Extend `AudioSettings` with `bitrate_mode: enum { Cbr(u32 kbps), Vbr(u8 quality_level) }`.
  - If VBR: set `bit_rate=0`, set quality via unsafe `av_opt_set` (keys: `q`, `global_quality`, and flag `AV_CODEC_FLAG_QSCALE`). Start with mapping quality 1–5 where 2≈56 kbps mono, 3≈64, 4≈72 (tune after measurement).
  - Log chosen effective mode.
- Fallback: If opt set fails, revert to prior CBR path & log warn once.
Acceptance:
- Can switch mode via temporary env `ABB_AUDIO_MODE=VBR3` before UI exists.
- Logs confirm mode & any fallback.

## Phase 3 – Module Decomposition
Split `media_pipeline.rs`:
- `plan.rs` – `MediaProcessingPlan` + duration helper.
- `encoder.rs` – encoder creation, twoloop, VBR/CBR selection, finalize.
- `decoder.rs` – open input, create decoder + resampler.
- `buffer.rs` – (from Phase 1) accumulator.
- `process.rs` – orchestration: iterate files, packet loop, progress.
- `progress.rs` – `emit_progress_update` helper.
- `cover_art.rs` – native cover art stream + packet write.
Keep `media_pipeline.rs` as façade `pub use` + minimal `FfmpegNextProcessor` impl (<150 LOC).
Acceptance:
- No file > 350 LOC.
- No function > 55 LOC (except explicitly annotated EXCEPTION blocks if any remain — goal is zero exceptions).
- Removed obsolete comments (legacy references, over-verbose logging commentary).

## Phase 4 – UI & Settings Integration
Tasks:
- TS: Add `BitrateMode` type in `src/types/audio.ts`.
- UI: In output panel split, add Mode selector (CBR | VBR). If CBR: show numeric kbps input (existing). If VBR: hide kbps, show Quality slider (1–5) with helper text (approx kbps speech).
- Persist selection in existing settings state; send to Rust via command payload.
- Add validation: disallow switching mode mid-processing.
Acceptance:
- Selecting VBR updates preview size estimation (use heuristic table: estimated_bitrate_for_quality(q)).
- Command payload seen in Rust logs (mode=VBR3 etc.).

## Phase 5 – Testing & Metrics
Tasks:
- Unit tests: accumulator; VBR config fallback; encoder selection.
- Integration test: ensure output duration delta vs sum(input) < 0.1%.
- Performance test (simple timing) behind `#[ignore]`.
- Update docs: `docs/reports/audio_pipeline_refactor_rationale.md` (already prepared); add updated `AAC_advice.md` snippet referencing VBR.
Acceptance:
- `cargo test` green.
- Clippy no new warnings.

## Phase 6 – Cleanup & Hardening (Stretch)
- Consider gapless metadata (encoder delay / padding) if user reports pacing issues.
- Optionally introduce faster resampler config (e.g. `swr_set_compensation` tuning) if profiling shows resample bottleneck.

## Rollback Strategy
- Keep branch per phase (`p12/phase-1-accumulator`, etc.).
- Accumulator guarded by feature flag `ABB_ACCUMULATOR` for rapid revert during initial test (removed after Phase 2 if stable).

## Task Assignment (You vs Me)
- You: Run baseline (Phase 0), review diffs, UI wiring.
- Me: Implement Phases 1–3 core Rust, provide code reviews & VBR unsafe glue.

## Acceptance Summary Checklist
- [ ] Baseline metrics doc
- [ ] No truncation logs
- [ ] Audio sample loss <= 0.02%
- [ ] Throughput >= 2× baseline (Release build) or justified
- [ ] Module split complete (see list)
- [ ] CBR/VBR selectable (UI + backend)
- [ ] Tests added & green
- [ ] Rationale report updated

## Minimal Sequencing Dependencies
Accumulator (Phase 1) must precede VBR (Phase 2) for accurate quality comparisons.
Module split (Phase 3) after correctness improvements to shrink diff complexity.
UI (Phase 4) after backend VBR stable.

## Risk Notes
- Unsafe VBR opts may vary by FFmpeg build; mitigate with graceful fallback.
- Accumulator memory: worst case a single oversized decoded frame; bounded (< 2 * frame_size * channels * 4 bytes).

## Go Signal
Proceed directly with Phase 1 implementation next.
