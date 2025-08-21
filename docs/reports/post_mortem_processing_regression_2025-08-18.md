## Processing Regression Post‑Mortem (2025‑08‑18)

Note: This repository is ffmpeg-next only; any references to shell FFmpeg are historical context.

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


# Regression Guard Strategy

# Audio Processing Regression Guard (Parking Lot Draft)

Purpose: Snapshot of proposed layers to prevent "no output / encoder error" regressions. NOT active yet. Review later when ready.

## 0. Problem Statement
Too many regressions where processing aborts (no M4B) due to low-level pipeline mistakes (missing alloc, frame_size=0 handling, truncation, etc.). Need a fast, reliable tripwire BEFORE merge.

## 1. Minimum Viable Guard (Small, Fast, Just Enough)
1. Smoke Encode Test (E2E):
   - Input: 2–3 tiny MP3 fixtures.
   - Output: single M4B.
   - Checks:
     - Output file exists & >0 bytes.
     - Duration within ±0.5s of sum input durations.
     - Log scan: no lines containing (case-insensitive) `encoder send failed`, `NaN`, `Invalid argument`.
2. Allocation & NaN Guards:
   - Before sending to encoder: assert frame planes allocated & first/last sample finite (debug build only).
3. Golden Manifest (Sample & Duration Baseline):
   - JSON recorded once from a known good run: `{ total_input_samples, total_encoded_samples, duration_s }`.
   - Test compares: sample delta <= frame_size (tail pad), duration delta < 0.5%.

That’s the core. If we only do this, we catch the class of failures seen so far.

## 2. Golden Run Concept (Plain English)
"Golden run" = Save authoritative reference metrics from a clean, correct encode. Future runs must match within small tolerances. If they don’t, the test fails and blocks merge. It protects against:
- Silent sample loss / duplication.
- Broken frame sizing causing large duration drift.
- Accidental format / container changes.

Why small JSON vs hashing whole file? File-level hashes break on benign metadata byte differences. Metrics are stable and intentional.

## 3. Tolerances (Initial)
- Sample count delta: <= encoder_frame_size (1024) overall.
- Duration delta: < 0.5% or <0.5s (whichever larger).
- If either exceeded → FAIL with diagnostic print of actual vs golden.

## 4. Test File Layout (Proposed)
```
src-tauri/tests/
  processing_smoke.rs          # orchestrates encode & invariants
  fixtures/
    input1.mp3
    input2.mp3
  golden/
    baseline_manifest.json
```
`baseline_manifest.json` added only after a verified green manual run.

## 5. Developer Flow
1. Change audio code.
2. Run `cargo test --quiet processing_smoke`.
3. If intentional sample/duration change: re-run with `GOLDEN_APPROVE=1` to regenerate manifest (explicit opt-in).
4. Commit both code + updated manifest with note explaining reason.

## 6. Simple Invariants to Code
- Frame destined for resampler MUST be allocated (planes length > 0) after alloc.
- No NaN/Inf samples (debug assert).
- If encoder.frame_size == 0 -> log fallback size exactly once.

## 7. Nice-to-Have (Defer)
- Performance timing (warn >15% regression).
- Property test: accumulator reconstructs sample sequence.
- Fuzz malformed MP3 headers (weekly job).

## 8. Exit Criteria for Accepting a PR that Touches Pipeline
- Smoke test passes.
- Golden manifest unchanged OR re-approved.
- No new warn/errors inserted (clippy clean for touched code).

## 9. Why This Is Lean
- Adds < ~3s to test run.
- Minimal fixtures; no huge binaries.
- JSON diff easy to review in PR.

## 10. Next Step (When Ready)
Implement ONLY Section 1 + 2 + write baseline manifest. Everything else waits until explicitly re-prioritized.

---
(End of parking lot doc – safe to ignore until you say “activate it”).

## 11. Manifest Schema (Concrete Example)
Minimal JSON stored in `tests/golden/baseline_manifest.json`:
```json
{
  "version": 1,
  "encoder_mode": "AAC-LC-CBR-64k",
  "sample_rate": 44100,
  "channels": 2,
  "frame_size_assumed": 1024,
  "inputs": [
    { "seconds": 1.234, "samples": 54432 },
    { "seconds": 1.229, "samples": 54192 }
  ],
  "expected_total_seconds": 2.463,
  "encoded_samples": 108624
}
```
Comparison uses integer `encoded_samples` as primary truth; duration is derived check with tolerance.

## 12. Additional Invariants / Guards (Not Yet Implemented)
1. Fallback Frame Size Log: If `encoder.frame_size()==0` we log once: `frame_size=0 fallback=1024` and test asserts its presence.
2. Negative Log Substrings (smoke test rejects case-insensitive): `encoder send failed`, `nan`, `invalid argument`, `panic`.
3. Golden Manifest Update is explicit: requires env var `GOLDEN_APPROVE=1` to overwrite; otherwise test fails if schema mismatch or values drift.
4. Finite Sample Guard (debug): first & last 64 samples of each plane must be finite; else panic in debug, error in release (decision pending).

## 13. Failure → Guard Matrix
| Failure Type | Guard Layer | Detection Mechanism |
|--------------|-------------|---------------------|
| Missing frame alloc (NaN samples) | Smoke + Alloc helper + Finite assert | Encoder error prevented; assert triggers early |
| Frame size 0 infinite loop risk | Invariant + log assert | Presence of fallback log + bounded accumulator |
| Silent sample truncation | Golden manifest | Sample delta > frame_size triggers fail |
| Dropped input file | Golden + Smoke (duration) | Total samples/duration shrink |
| Wrong sample rate/channels | Golden manifest | Exact field mismatch |
| Encoder mode accidentally changed | Golden manifest | `encoder_mode` mismatch |
| Cover art removal regression (optional) | Extended smoke (future) | Tag presence check |
| Excessive logging hides errors | Later log throttle | INFO spam reduced; error scan high signal |

## 14. ffmpeg-next Cheat Sheet (Memory / Frames)
- `frame.set_samples(n)` does NOT allocate buffers – must call `frame.alloc(sample_format, n, channel_layout)`.
- Resampler writes into destination frame's allocated planes; it will not auto-alloc.
- AAC encoder may report `frame_size=0` (variable frame) – choose canonical accumulation size (1024) for consistency.
- Validate planes: length in bytes ≥ samples * bytes_per_sample; channel count matches layout.
- Never pass unallocated or zero-sample frames to encoder.

## 15. Minimal Activation Checklist (When Resuming)
1. Implement resample allocation helper + replace inline logic.
2. Add smoke test (2 tiny MP3 fixtures or base64-embedded) with log scanning.
3. Capture baseline manifest via `GOLDEN_APPROVE=1` run.
4. Add finite sample debug assert & fallback frame_size log.
5. Stop. (Golden + Smoke + invariants now guard regressions.)

(Supplemental sections 11–15 added for future activation; still parked.)