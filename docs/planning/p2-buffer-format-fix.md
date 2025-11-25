# Plan: Fix SampleAccumulator Format Mismatch for AAC-AT

## Session Progress (PR1 + PR2 completed)

### Items to mark COMPLETED in `docs/planning/progress_bug_tracker.md`:

- **P1-A: Processing fails silently - output path contract mismatch** → DONE
  - Fixed in `audio.rs`: `prepare_output_path()` now accepts full file path, creates parent directories

- **P1-B: Bitrate range mismatch** → DONE
  - Expanded to [48, 56, 64, 72, 80, 88, 96, 112, 128] kbps
  - Updated: `settings_encoder.rs`, `settings.rs`, `audio.ts`, `encoder.ts`, `logic.ts`, `index.html`

- **P1-C: Sample rate not passed** → DONE
  - Added `sample_rate: Option<SampleRateConfig>` to `ProcessV2Payload`
  - Maps through to `settings_v1.sample_rate` in command handler

### New issue discovered this session:
- **BUG: SampleAccumulator format mismatch with AAC-AT** (this plan)

---

## Problem Summary

After fixing the AAC-AT encoder to use S16 sample format (instead of F32 planar), the `SampleAccumulator` in `buffer.rs` is now mishandling audio data because it's hardcoded for f32 planar format.

### Symptoms (from console output)
1. **Sample count mismatch every packet:**
   ```
   Frame plane 0 has fewer samples than reported (have=576, expected=1152)
   ```
   - 576 = 1152 samples × 2 bytes (S16) ÷ 4 bytes (f32 assumption)
   - Accumulator divides byte length by 4 (f32), but data is S16 (2 bytes)

2. **High sanitization rate (30-70% of samples):**
   ```
   Accumulator sanitized 553 samples before encoding (frame_size=1024)
   ```
   - S16 integer bytes interpreted as f32 floats appear as garbage values
   - Most raw bytes fail the `is_finite()` or `[-1, 1]` range checks

### Root Cause

`SampleAccumulator` (`buffer.rs:13`) uses `Vec<Vec<f32>>` internally and assumes:
- 4 bytes per sample (line 53: `plane.len() / 4`)
- Planar layout (separate buffer per channel)
- Float values in [-1.0, 1.0] range

But AAC-AT requires S16 packed format:
- 2 bytes per sample
- Interleaved layout (all channels in one buffer)
- Integer values in [-32768, 32767] range

## Recommended Fix

### Option A: Format-Aware Accumulator (Recommended)

Make `SampleAccumulator` aware of the encoder's sample format:

1. Store the format in the accumulator struct
2. Calculate bytes-per-sample based on format
3. Handle packed vs planar layouts
4. Skip float sanitization for integer formats (or convert to appropriate range check)

**Pros:** Clean solution, single accumulator handles both formats
**Cons:** More complex implementation

### Option B: Bypass Accumulator for S16

For S16 packed format, pass frames directly to encoder without accumulation:

1. Check format before using accumulator
2. If S16 packed, send frames directly (AAC-AT handles variable sizes)
3. Keep accumulator only for f32 planar path

**Pros:** Minimal changes, quick fix
**Cons:** Divergent code paths, may miss edge cases

## Implementation Steps (Option A)

### Step 1: Update SampleAccumulator struct
- Add `bytes_per_sample: usize` field
- Add `is_planar: bool` field
- Calculate from format in constructor

### Step 2: Fix sample counting
- Line 53: Change `plane.len() / 4` to `plane.len() / self.bytes_per_sample`

### Step 3: Handle packed format
- For packed: single buffer with interleaved samples
- For planar: current multi-buffer approach

### Step 4: Format-appropriate sanitization
- F32: current [-1.0, 1.0] clamping
- S16: check [-32768, 32767] range (or skip sanitization - integers can't be NaN)

### Step 5: Output frame construction
- Set correct format on output frames
- Handle both packed and planar allocation

## Files to Modify

| File | Changes |
|------|---------|
| `src-tauri/src/audio/buffer.rs` | Format-aware accumulator |
| `src-tauri/src/audio/processor/frame_pipeline.rs` | Pass format info to accumulator |

## Verification

- [ ] `cargo test` passes
- [ ] `quick-checks.sh` passes
- [ ] Process audiobook with AAC-AT: no sample warnings
- [ ] Process audiobook with native AAC (F32): still works
- [ ] Output file plays correctly in audio player

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking F32 path | Keep existing f32 logic, add S16 branch |
| Incorrect byte calculation | Add debug assertions for sample counts |
| Audio quality degradation | Test output with ffprobe, listen test |
