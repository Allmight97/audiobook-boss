- Agreed and noted: no new code in `media_pipeline.rs` unless strictly fixing; we’ll prioritize refactoring it first [[memory:6470341]].

### Why we’re close
- With fast-path disabled, processing runs for a long time and produces a valid `.m4b` (cover art intact). That isolates remaining instability to the fast-path (decoder → accumulator → encoder) and frame boundary semantics, not to container/metadata or decoder/resampler.

### Tactical, atomized refactor plan (Godzilla breakup)
Small, sequential edits with compile+tests after each step. No behavior changes until the end, except where noted.

1) Centralize sample sanitation
   - Move all clamp/finite handling into `audio/buffer.rs` only; remove duplicate sanitize from `encode_and_write_frame`.
   - Acceptance: identical waveforms (within clamp); no encoder NaN/Inf; tests green.

1b) Frame contract validator (debug‑only)
   - Add a small validator that asserts, in debug builds, that frames going into the encoder are: allocated, planar layout matches encoder, `nb_samples` > 0 and ≤ `frame_size` (or accumulator chunk), all samples finite and within [-1, 1], and PTS monotonic.
   - Acceptance: in dev, any violation trips immediately with a high‑signal error; in release, no overhead.

2) Extract encoder concerns
   - New `audio/processor/encoder.rs`:
     - `create_audio_encoder`, `setup_encoder`, `finalize_encoding`, `encode_and_write_frame`.
   - `media_pipeline.rs` calls into this module.
   - Acceptance: compile + cargo test; logs unchanged.

3) Extract decode+resample frame pipeline
   - New `audio/processor/frame_pipeline.rs`:
     - `process_input_file`, `process_input_packets`, `process_decoded_frames`.
   - Keep PTS/progress emission behavior intact.
   - Acceptance: compile + tests; progress events still flow.

3b) Fast‑path policy
   - Keep `ABB_DISABLE_FASTPATH=1` default until the new modules land and pass long‑run tests. Track a checklist to re‑enable by default (contract tests green, long‑run stability, performance delta measured, rollback toggle retained).

4) Extract decoder+resampler setup
   - New `audio/processor/streams.rs`:
     - `setup_decoder_and_resampler`.
   - Acceptance: logs for input/output formats unchanged.

5) Isolate cover-art orchestration
   - New `audio/processor/cover_art.rs`:
     - Pre/post header stream add/write decisions (still using `metadata::ffmpeg_bridge` functions).
   - Acceptance: native cover art still embedded; fallback still works.

6) Keep orchestrator tiny
   - `FfmpegNextProcessor` remains but delegations only; file < 250 LOC.
   - Acceptance: `media_pipeline.rs` shrinks, high-level flow readable.

7) Observability pass (no new features)
   - Promote key logs to new modules; reduce noisy per-packet logs.
   - Acceptance: same informational value, lower verbosity.

7b) Tests as safety net
   - Add property/golden tests: MP3 VBR/CBR, mono/stereo, 44.1/48k. Fuzz injects NaN/out‑of‑range into accumulator to ensure sanitize + validator catch issues.
   - Compare golden outputs against shell FFmpeg reference where feasible (PCM segment hash or spectral stats), documented in `docs/reports`.

8) Remove fast-path env toggle after stabilization
   - Re-enable only when accumulator path proves stable in long runs; keep the toggle for emergency rollback.

9) Lint + docs
   - `cargo clippy -- -D warnings`; brief README snippet: `ABB_DISABLE_FASTPATH=1` recommended until fast-path stabilized.

### Handling the “settings not honored” regression
- Triage note: Likely in parameter resolution vs UI settings mapping (e.g., `SampleRateConfig::Auto` vs `Explicit`, channels). We’ll file a follow-up P1 and address right after the refactor:
  - Log at start: resolved `target_sample_rate`, `target_channels`, `bitrate` and the UI `settings` to verify the path.
  - Ensure `resolve_target_audio_params` respects explicit UI values.
  - Verify resampler/encoder actually uses those resolved params.

### What to do now
- Keep `ABB_DISABLE_FASTPATH=1` for reliability while we execute Steps 1–3.
- I’ll start with Step 1–3 in small edits, running full tests between each.

- We’ll fix the settings regression right after the breakup (tracked as P1), then re-evaluate enabling the fast-path by default.

- Impact
  - Safer, testable modules; easier to reason about the fast-path vs resampler behavior.
  - Faster iteration on the settings regression and performance tuning.

- Summary:
  - We’ll split `media_pipeline.rs` into focused modules for encoder, stream setup, and frame pipeline, centralize sanitation in `buffer.rs`, keep fast-path off for stability, and address the output-settings regression immediately after the breakup.