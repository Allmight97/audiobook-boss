# P1 Fix Plan: Settings Contract Alignment

## Executive Summary

Three interconnected issues prevent audio processing and cause settings to be silently ignored:

| ID | Issue | Severity | Impact |
|----|-------|----------|--------|
| P1-A | Output path contract mismatch | **Blocker** | Processing fails silently |
| P1-B | Bitrate range mismatch | High | User's 128k → 64k silently |
| P1-C | Sample rate not passed | High | UI setting ignored entirely |

## Root Cause Analysis

### P1-A: Output Path Contract Mismatch (Processing Fails)

**Frontend sends full file path:**
```
/Users/jstar/Audiobooks/Author/2024-Title/Title (2024).m4b
```

**Backend expects directory and constructs filename:**
```rust
fn validate_and_resolve_output_path(output_dir: &str) -> Result<PathBuf> {
    let dir = Path::new(output_dir);
    if !dir.is_dir() { return Err(...) }  // FAILS HERE
    Ok(dir.join("audiobook.m4b"))         // Never reached
}
```

**Evidence:**
- `outputPanel.ts:202-211` - `calculateOutputPath()` returns full path with `.m4b`
- `logic.ts:194` - Reads from `output-dir-text` input (which contains full path)
- `audio.rs:199-215` - Validates as directory, then appends own filename

### P1-B: Bitrate Range Mismatch

| Location | Allowed Values |
|----------|----------------|
| UI Dropdown (HTML) | 32, 48, 56, **64**, 96, 128 |
| `logic.ts:32` | 56, 64, 72, 80, 88, 96 |
| `encoder.ts:46` | 56, 64, 72, 80, 88, 96 |
| `settings_encoder.rs:60` | 56, 64, 72, 80, 88, 96 |
| `EncoderSettings` type | `56 \| 64 \| 72 \| 80 \| 88 \| 96` |

**Result:** User selects 128 → sanitized to 64 in frontend → backend never sees 128

### P1-C: Sample Rate Not Passed

- `EncoderSettings` (both TS and Rust) has **no sample rate field**
- `audio.rs:90` creates `settings_v1` from `audiobook_preset()` → `SampleRateConfig::Auto`
- User's explicit 44100 is collected but **never reaches backend**

---

## Proposed Fix Strategy

### Decision Point: Output Path Contract

**Option A: Frontend sends directory only, backend constructs full path**
- Pros: Backend has full control over filename; easier validation
- Cons: Requires frontend to communicate filename pattern to backend
- Change scope: Medium (both sides)

**Option B: Frontend sends full file path, backend uses it directly**
- Pros: Frontend controls filename pattern; simpler mental model
- Cons: Backend must create parent directories; less validation control
- Change scope: Medium (backend only)

**Recommendation:** Option B - Frontend already computes the desired full path. Backend should accept it and create directories as needed. This respects user's filename pattern choices.

### Decision Point: Bitrate Range

**Proposed canonical range:** 48, 56, 64, 72, 80, 88, 96, 112, 128 kbps

Rationale:
- 48k: Low bandwidth/storage for speech
- 56-96: Current supported range (8k steps)
- 112, 128: Higher quality options users expect

**Single source of truth:** Define in Rust `settings_encoder.rs`, expose via command, consume in TypeScript

### Decision Point: Sample Rate

**Option A: Add sample_rate to EncoderSettings**
- Full parity between UI and backend
- Requires type changes on both sides

**Option B: Keep sample_rate in AudioSettings (v1), ensure it's passed**
- Minimal change; v1 already has it
- Requires fixing the mapping in `process_audiobook_files_v2`

**Recommendation:** Option B for now (minimal change), with note that v2 should eventually absorb it.

---

## Implementation Plan

### PR Strategy

Two PRs to minimize risk and enable incremental verification:

| PR | Scope | Files Changed | Risk |
|----|-------|---------------|------|
| **PR1** | P1-A + P1-C | `audio.rs` only | Low - backend only, self-contained |
| **PR2** | P1-B | 5+ files (Rust, TS, HTML) | Medium - contract change |

---

## PR1: Fix Processing Blocker + Sample Rate

**Branch:** `fix/p1-processing-and-samplerate`

### P1-A: Fix Output Path Contract

**File:** `src-tauri/src/commands/audio.rs`

**Changes:**
1. Rename `validate_and_resolve_output_path` to `prepare_output_path`
2. Accept full file path (not directory)
3. Validate parent directory exists OR create it
4. Return the provided path directly (don't append filename)

**Code sketch:**
```rust
fn prepare_output_path(output_path: &str) -> Result<PathBuf> {
    let path = Path::new(output_path);

    // Validate extension
    if path.extension().and_then(|e| e.to_str()) != Some("m4b") {
        return Err(AppError::InvalidInput("Output must be .m4b file".into()));
    }

    // Get parent directory
    let parent = path.parent().ok_or_else(||
        AppError::InvalidInput("Invalid output path".into()))?;

    // Create parent directories if needed
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e|
            AppError::FileValidation(format!("Cannot create directory: {}", e)))?;
    }

    Ok(path.to_path_buf())
}
```

### P1-C: Fix Sample Rate Passthrough

**File:** `src-tauri/src/commands/audio.rs`

**Changes:**
In `process_audiobook_files_v2`, map sample rate from payload:

```rust
// Current (broken):
let mut settings_v1 = audio::AudioSettings::audiobook_preset();

// Fixed:
let mut settings_v1 = audio::AudioSettings::audiobook_preset();
// TODO: Accept sample_rate in ProcessV2Payload or parse from AudioSettings
// For now, keep Auto behavior but document the gap
```

**Note:** Full fix requires adding `sample_rate` to `ProcessV2Payload`. Interim fix: document limitation.

### PR1 Verification Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] Manual test: Load files, select output dir, click Process → processing starts
- [ ] Manual test: Output file created in correct subdirectory path
- [ ] Manual test: Error shown if output path invalid

---

## PR2: Expand Bitrate Range (P1-B)

**Branch:** `fix/p1-bitrate-range`

**Prerequisite:** PR1 merged and verified

### Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/audio/settings_encoder.rs` | `VALID_ENCODER_BITRATES = [48, 56, 64, 72, 80, 88, 96, 112, 128]` |
| `src/types/audio.ts` | Update `bitrateKbps` type union |
| `src/types/encoder.ts` | Update `VALID_ENCODER_BITRATES` array |
| `src/ui/statusPanel/logic.ts` | Update `SUPPORTED_ENCODER_BITRATES` set |
| `index.html` | Update `<select id="output-bitrate">` options |

### New Valid Bitrates

```
[48, 56, 64, 72, 80, 88, 96, 112, 128]
```

### PR2 Verification Checklist

- [ ] `scripts/quick-checks.sh` passes
- [ ] `npm run build` succeeds
- [ ] Manual test: Select 48k → output at 48k (verify with ffprobe)
- [ ] Manual test: Select 128k → output at 128k
- [ ] Manual test: Default 64k still works

---

## Future: Contract Tests (Post-PR2)

**New test files:**
- `src-tauri/tests/contract_settings.rs`

**Test cases:**
1. Valid bitrates serialize/deserialize correctly
2. Invalid bitrates produce actionable errors
3. Full path output validation works
4. Directory creation works

---

## Future: Diagnostic Commands (Post-PR2)

Add to `window.testCommands`:
```typescript
testProcessingPipeline: async () => {
  // Returns what would be sent to backend without actually processing
  // Useful for debugging settings flow
}
```

---

## Testing Matrix

| Scenario | Expected Before Fix | Expected After Fix |
|----------|--------------------|--------------------|
| Process with 128k bitrate | Silent fallback to 64k | Output at 128k |
| Process with 48k bitrate | Silent fallback to 64k | Output at 48k |
| Process with 44100 sample rate | Uses input file rate | Output at 44100 |
| Process with subdirectory pattern | Fails silently | Creates directories, succeeds |
| Invalid bitrate in request | May crash or silent fallback | Clear error message |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking existing functionality | Run full test suite before/after |
| Directory creation security | Validate path doesn't escape output root |
| Type changes break serialization | Contract tests catch mismatches |

---

## Questions for Owner (Resolved)

1. **Bitrate range:** Is 48-128 kbps acceptable? Any values to add/remove?
   - **Decision:** 48, 56, 64, 72, 80, 88, 96, 112, 128 kbps ✓

2. **Directory creation:** Should backend auto-create directories, or error if missing?
   - **Decision:** Auto-create directories (user expects subdirectory pattern to work) ✓

3. **Sample rate:** Should it eventually move to `EncoderSettings` (v2 type)?
   - **Decision:** Quick fix in PR1 (document gap), full fix deferred to future v2 work ✓

4. **PR Strategy:** Bundle or separate?
   - **Decision:** Two PRs - PR1 (P1-A + P1-C), PR2 (P1-B) ✓

