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
