## Processing Regression Post‑Mortem (2025‑08‑18)

### Executive summary
- **Symptom**: Pressing Process did not start processing; terminal showed no meaningful progress or aborted early with AAC encoder errors.
- **Primary finding**: The encoder received invalid audio frames ("Input contains (near) NaN/+-Inf"). This surfaced most reliably when the "fast‑path" (decoder → accumulator → encoder) bypassed the resampler.
- **Current mitigation**: Disable the fast‑path (use the resampler path) which reliably completes and produces a valid `.m4b` with intact cover art. Keep fast‑path off while we refactor and fix the underlying issue.
- **New regression**: Output audio properties selected in the UI aren’t being honored in the produced file; we will address this immediately after the refactor.

### Timeline (condensed)
- Earlier working runs (see `docs/reports/latest_terminal_output.md`) showed explicit frame shaping: "Truncating frame from 1152 to 1024 samples for AAC" and completed successfully.
- Recent runs showed encoder failures early in processing: `[aac] Input contains (near) NaN/+-Inf` with `Encoder send failed: Invalid argument`.
- Fix 1: Allocated destination audio frames before resampling in `src-tauri/src/audio/media_pipeline.rs` to avoid uninitialized buffers. Tests passed, behavior improved but failures persisted on the fast‑path.
- Fix 2: Tightened copy bounds and added clamping/sanitization in `src-tauri/src/audio/buffer.rs` (accumulator) to avoid non‑finite/out‑of‑range values.
- Diagnostic: Introduced a fast‑path toggle (`ABB_DISABLE_FASTPATH=1`). With fast‑path OFF, long runs succeeded, producing a valid `.m4b` (cover art embedded). With fast‑path ON, failures recurred quickly.
- Observation: Even with resampler path, one test run later still failed late in processing, indicating lingering edge cases; unconditional sanitize in the encode boundary improved resilience but the safest path remains: fast‑path OFF until refactor and deeper fix.

### Technical analysis
- The encoder error indicates frames with invalid samples reached `avcodec_send_frame`. Sources observed:
  - Missing output frame allocation before resampling (now fixed).
  - Accumulator path (fast‑path) assembling frames directly for the encoder. Risk areas:
    - Plane/sample count assumptions when converting planar `f32` data into encoder frames.
    - Frame size alignment and `nb_samples` across channels.
    - Extremely out‑of‑range samples or denormals slipping through (now clamped/sanitized in `buffer.rs`).
- Why resampler path is more robust: The resampler produces consistently aligned frames and seems to smooth/normalize edge conditions that the direct accumulator path exposes.

### Mitigation in place
- Default to resampler path for reliability (set `ABB_DISABLE_FASTPATH=1`).
- Centralized sample clamping/finite‑sanitization in `src-tauri/src/audio/buffer.rs` to ensure frames emitted to the encoder are safe.
- Kept logs for single‑line repair counts to maintain observability without noise.

### Remaining issues
- Output audio settings (rate/channels/bitrate) not honored: The produced output doesn’t reflect panel selections.
  - Likely locus: Parameter resolution in `resolve_target_audio_params` vs. UI `AudioSettings`, and consistent application of those parameters in encoder/resampler setup.
  - Next steps: Verify explicit settings path vs `Auto`, ensure encoder/resampler use the resolved values, and add debug logs at encoder creation to print the resolved target parameters.

### Root cause status (have we found the primitive?)
- We have identified the problematic path (fast‑path accumulator → encoder) and removed one clear defect (unallocated resampler output), but we have not yet isolated a single primitive that conclusively explains all NaN/Inf occurrences under the fast‑path.
- Working hypothesis: The accumulator’s direct frame construction occasionally violates implicit encoder expectations (sample count per channel/frame boundary/stride), producing invalid input frames even after basic sanitization. Refactoring to isolate concerns will make this testable and fixable with precision.

### Refactor plan (brief)
- Break up `src-tauri/src/audio/media_pipeline.rs` into:
  - `audio/processor/encoder.rs` (encoder setup/finalize/write)
  - `audio/processor/streams.rs` (decoder/resampler setup)
  - `audio/processor/frame_pipeline.rs` (decode→resample→accumulate→encode)
- Keep sample sanitation solely in the accumulator module (`buffer.rs`).
- Maintain `ABB_DISABLE_FASTPATH=1` until the fast‑path is proven stable; retain the toggle for rollback.

### Commands used during mitigation
```bash
RUST_LOG=debug npm run tauri dev
ABB_DISABLE_TWOOLOOP=1 RUST_LOG=debug npm run tauri dev
ABB_DISABLE_FASTPATH=1 RUST_LOG=debug npm run tauri dev
```

### Lessons learned
- When introducing a new path (fast‑path), keep an explicit feature toggle and robust diagnostics.
- Always allocate destination audio frames before resampling to avoid UB/non‑finite propagation.
- Centralize data sanitation where frames are constructed to avoid divergent behavior across paths.

### Architectural contributing factors
- Shell FFmpeg previously enforced many invariants implicitly (frame sizing, allocation, valid ranges). Migrating to ffmpeg‑next moved those responsibilities into our code without all invariants being explicitly codified.
- A large, multi‑responsibility module (`media_pipeline.rs`) obscured boundaries (decode/resample/accumulate/encode/progress/cover art), making it hard to pinpoint where invariants were violated.
- The fast‑path bypassed the resampler’s normalizing behavior and exposed subtle frame‑shaping issues (nb_samples/alignment/planar layout) that weren’t enforced by tests.
- Missing contract tests and a "frame contract" (what a valid frame to the encoder means) allowed regressions to slip in.

### Design and policy updates
- Safe default: keep the resampler path ON by default; retain a fast‑path feature flag for controlled rollout.
- Explicit frame contract: document and validate, at module boundaries, the requirements for frames handed to the encoder (allocation done, planar layout, nb_samples aligned, finite/clamped amplitudes, monotonic PTS).
- Centralized sanitation: only the accumulator constructs/clamps/sanitizes frames; remove encode‑layer sanitization after refactor to avoid duplication.
- Debug‑only validator: add a lightweight frame validator (behind debug assertions) before encoding to fail fast during development without runtime cost in release.
- Tests as safety net: add property/golden tests across MP3 VBR/CBR, mono/stereo, common rates; add fuzz tests injecting NaN/out‑of‑range samples into the accumulator path; add golden comparisons vs shell FFmpeg outputs.

### Action items
- Execute the media pipeline refactor (incremental, with tests after each step).
- Triage and fix the output settings not honored (P1) immediately after the breakup.
- Re‑enable fast‑path and validate long‑run stability before making it default.


