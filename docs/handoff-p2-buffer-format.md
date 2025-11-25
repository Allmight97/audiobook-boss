# Handoff: P2 Buffer Format Fix (Session 2024-11-24)

## PR Created
- **PR #28**: [fix(buffer): Make SampleAccumulator format-aware for S16 packed support](https://github.com/Allmight97/audiobook-boss/pull/28)
- **Branch**: `fix/p2-buffer-format`
- **Status**: Ready for review

## Problem Solved

After fixing the AAC-AT encoder to use S16 sample format (PR from previous session), the `SampleAccumulator` in `buffer.rs` was mishandling audio data because it was hardcoded for f32 planar format.

### Symptoms (from console output)
1. **Sample count mismatch every packet:**
   ```
   Frame plane 0 has fewer samples than reported (have=576, expected=1152)
   ```
   - 576 = 1152 samples × 2 bytes (S16) ÷ 4 bytes (f32 assumption)

2. **High sanitization rate (30-70% of samples):**
   ```
   Accumulator sanitized 553 samples before encoding (frame_size=1024)
   ```
   - S16 integer bytes interpreted as f32 floats appear as garbage values

### Root Cause
`SampleAccumulator` (`buffer.rs:13`) used `Vec<Vec<f32>>` internally and assumed:
- 4 bytes per sample (line 53: `plane.len() / 4`)
- Planar layout (separate buffer per channel)
- Float values in [-1.0, 1.0] range

But AAC-AT requires S16 packed format:
- 2 bytes per sample
- Interleaved layout (all channels in one buffer)
- Integer values in [-32768, 32767] range

## Solution Implemented

### Changes to `src-tauri/src/audio/buffer.rs`

1. **Added `SampleStorage` enum** (lines 11-14):
   ```rust
   enum SampleStorage {
       F32Planar(Vec<Vec<f32>>),
       S16Packed(Vec<i16>), // Interleaved: [L0, R0, L1, R1, ...]
   }
   ```

2. **Format-aware constructor** - Calculates `bytes_per_sample` from format:
   - U8: 1 byte
   - I16: 2 bytes
   - I32/F32: 4 bytes
   - F64: 8 bytes

3. **Separate drain methods**:
   - `drain_one_f32_planar()` - Original logic with float sanitization
   - `drain_one_s16_packed()` - Direct copy, no sanitization (integers can't be NaN)

4. **New tests added**:
   - `accumulator_s16_packed_mono`
   - `accumulator_s16_packed_stereo`
   - `accumulator_s16_partial_flush`

## Verification Status

| Check | Status |
|-------|--------|
| `cargo test` (122 tests) | PASS |
| `quick-checks.sh` | PASS |
| Manual test: AAC-AT processing | PENDING |
| Manual test: Native AAC (F32) | PENDING |
| Listen test: output quality | PENDING |

## Files Modified

| File | Changes |
|------|---------|
| `src-tauri/src/audio/buffer.rs` | +377/-36 lines - Format-aware accumulator |

## Related Documentation

- **Plan file**: `docs/planning/p2-buffer-format-fix.md`
- **Progress tracker**: `docs/planning/progress_bug_tracker.md` (P1-A, P1-B, P1-C marked complete in previous session)

## Pending Items

1. **Manual verification** - Process an audiobook with AAC-AT encoder and verify:
   - No "fewer samples than reported" warnings
   - No "sanitized X samples" warnings
   - Output file plays correctly

2. **Update progress tracker** - After PR merge, mark P2 as complete in `progress_bug_tracker.md`

## Session Context for Next Pickup

- The PR implements Option A from the plan (format-aware accumulator)
- All automated tests pass
- Ready for Gemini feedback review and manual testing
- The encoder fix from the previous session (S16 format selection for aac_at) is already merged

## Quick Commands

```bash
# Run tests
cd src-tauri && cargo test

# Run quick checks
./scripts/quick-checks.sh

# Test with verbose logging
RUST_LOG=debug npm run tauri dev

# View PR
gh pr view 28
```
