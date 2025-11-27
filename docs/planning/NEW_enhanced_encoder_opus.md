# Enhanced Encoder Engine Plan

## Overview
Replace the current encoder with a production-ready engine based on `shrink.sh` patterns, adding FDK HE-AAC, Apple AAC CVBR, and Opus encoders with an enhanced multi-file adaptive preview system.

## Scope Summary - Split into 2 PRs

### PR 1: Encoder Engine + UI
1. **Encoder Types**: FDK VBR + afterburner, Apple CVBR, Native AAC, Opus
2. **Detection**: Auto-detect available encoders + custom FFmpeg path for FDK
3. **UI Updates**: Encoder selection, VBR controls, feature flag enablement
4. **Target**: `feature/enhanced-encoder` → `new_encoder`

### PR 2: Adaptive Preview Enhancement
1. **Multi-file adaptive preview**: Total duration ÷ file count = per-file excerpt
2. **Preview duration options**: Add 15s, 45s, 60s (30s remains default)
3. **Unified logic**: Same algorithm for 1 file or N files
4. **Target**: `feature/adaptive-preview` → `new_encoder`

---

## Part 1: Encoder Engine Enhancements

### 1.1 New Encoder Types (Rust)
**File**: `src-tauri/src/audio/settings_encoder.rs`

Update `EncoderType` enum:
```rust
pub enum EncoderType {
    Auto,           // NEW: Auto-detect best available (FDK > Apple > Native)
    FdkHeAac,       // NEW: libfdk_aac HE-AAC with VBR + afterburner
    AacAt,          // Existing: Apple AudioToolbox CVBR
    NativeAac,      // RENAMED from HeAacV1: FFmpeg native AAC
    Opus,           // NEW: libopus for Audiobookshelf compatibility
}
```

Add VBR settings:
```rust
pub enum BitrateMode {
    Cbr,            // Constant bitrate
    Vbr(u8),        // Variable bitrate quality 1-5 (FDK only)
    Cvbr,           // Constrained VBR (Apple aac_at)
}
```

Update `EncoderSettings`:
```rust
pub struct EncoderSettings {
    pub encoder_type: EncoderType,
    pub bitrate_kbps: u16,
    pub bitrate_mode: BitrateMode,      // NEW
    pub channels: u8,
    pub afterburner: bool,              // FDK only
    pub custom_ffmpeg_path: Option<String>,  // NEW: User-provided FFmpeg with FDK
    pub threads: ThreadSetting,
}
```

### 1.2 Encoder Detection & Selection
**File**: `src-tauri/src/audio/settings_encoder.rs`

```rust
pub struct EncoderAvailability {
    pub fdk_available: bool,
    pub aac_at_available: bool,
    pub opus_available: bool,
    pub custom_ffmpeg_path: Option<PathBuf>,
}

pub fn detect_available_encoders(custom_path: Option<&str>) -> EncoderAvailability {
    // 1. Check system FFmpeg for libfdk_aac, aac_at, libopus
    // 2. If custom_path provided, probe that binary for libfdk_aac
    // 3. Return availability map
}

pub fn resolve_encoder(settings: &EncoderSettings, availability: &EncoderAvailability) -> ResolvedEncoder {
    // Auto mode: FDK > Apple (macOS) > Native AAC
    // Specific mode: Use requested or fallback with warning
}
```

### 1.3 FDK HE-AAC Configuration
**File**: `src-tauri/src/audio/processor/encoder.rs`

Wire up FDK-specific FFmpeg options (from `shrink.sh` defaults):
```rust
// FDK HE-AAC VBR mode with afterburner
// Default: VBR level 3 (~60kbps), afterburner enabled
fn configure_fdk_encoder(ctx: &mut EncoderContext, settings: &EncoderSettings) {
    // -c:a libfdk_aac
    // -profile:a aac_he
    // -vbr {1-5}  (from settings.bitrate_mode)
    // -afterburner 1 (if settings.afterburner)
}
```

### 1.4 Apple AAC CVBR Configuration
**File**: `src-tauri/src/audio/processor/encoder.rs`

```rust
// Apple aac_at with CVBR mode
// Default: CVBR @ 64kbps
fn configure_apple_encoder(ctx: &mut EncoderContext, settings: &EncoderSettings) {
    // -c:a aac_at
    // -aac_at_mode cvbr
    // -b:a {bitrate}
}
```

### 1.5 Opus Encoder Configuration
**File**: `src-tauri/src/audio/processor/encoder.rs`

```rust
// libopus with VBR (from opus.sh patterns)
// Default: 48kbps, compression_level 10, application audio
fn configure_opus_encoder(ctx: &mut EncoderContext, settings: &EncoderSettings) {
    // -c:a libopus
    // -b:a {bitrate}
    // -vbr on
    // -compression_level 10
    // -application audio
}
```

### 1.6 Defaults (from shrink.sh)
| Encoder | Profile | Bitrate Mode | Default Bitrate | Extra |
|---------|---------|--------------|-----------------|-------|
| FDK | HE-AAC | VBR 3 | ~60kbps | afterburner=1 |
| Apple | AAC | CVBR | 64kbps | - |
| Native | AAC-LC | CBR | 64kbps | twoloop coder |
| Opus | - | VBR | 48kbps | compression=10 |

---

## Part 2: Multi-File Adaptive Preview (PR 2)

### 2.1 Preview Strategy - Unified Approach
**Current**: First 30 seconds of merged output (effectively first file only)
**New**: Unified algorithm for 1 or N files - same logic, natural handling

**Algorithm from `shrink.sh` (lines 656-667):**
```
total_seconds = user_choice OR 30 (default)
min_segment_seconds = 5

per_file_seconds = total_seconds / file_count
if per_file_seconds < min_segment_seconds:
    per_file_seconds = min_segment_seconds
```

**Examples:**
- 1 file, 30s total → 30s from that file (same as current)
- 3 files, 30s total → 10s from each file
- 10 files, 30s total → 5s from each (floor at min_segment)
- 10 files, 60s total → 6s from each file

### 2.2 Preview Duration Options (UI)
**Current dropdown**: Just triggers 30s default
**New options**: 15s, 45s, 60s (30s is default when button pressed without selection)

**File**: `index.html` - Update preview dropdown options
**File**: `src/ui/statusPanel/logic.ts` - Wire up duration selection

### 2.3 Preview Config Enhancement
**File**: `src-tauri/src/audio/context.rs`

```rust
pub struct PreviewConfig {
    pub total_seconds: f64,           // User choice or 30s default
    pub min_segment_seconds: f64,     // Floor: 5s (hardcoded)
}
```

Note: No `adaptive` flag needed - always use same algorithm. 1 file case naturally gets full duration.

### 2.4 Frame Pipeline Changes
**File**: `src-tauri/src/audio/processor/frame_pipeline.rs`

The key insight: instead of early-stopping after total_seconds of merged audio, we need to:
1. **Calculate per-file duration** at start: `per_file_seconds = total / file_count`
2. **Track per-file elapsed time** during encoding
3. **Switch to next file** after extracting per_file_seconds
4. **Generate chapter marker** at each file boundary
5. **Stop** after all files processed OR total_seconds reached

New context fields:
```rust
struct PreviewState {
    file_count: usize,
    per_file_seconds: f64,
    current_file_index: usize,
    current_file_elapsed: f64,
    chapter_markers: Vec<ChapterMarker>,
}
```

### 2.5 Chapter Generation
**File**: `src-tauri/src/audio/processor/finalize.rs`

Generate FFMETADATA chapters (mirrors `shrink.sh` format):
```
;FFMETADATA1
[CHAPTER]
TIMEBASE=1/1000
START=0
END=10000
title=01 - Introduction
[CHAPTER]
TIMEBASE=1/1000
START=10000
END=20000
title=02 - Chapter One
...
```

Chapter titles derived from source filenames (sanitized).

### 2.6 Preview Output
- Output path: `{stem}.preview.m4b` (unchanged)
- Chapters embedded showing which source file each excerpt came from
- Single cohesive preview file regardless of input count

---

## Part 3: UI Updates

### 3.1 TypeScript Type Updates
**File**: `src/types/encoder.ts`

```typescript
export type EncoderType = 'auto' | 'fdk_he_aac' | 'aac_at' | 'native_aac' | 'opus';

export type BitrateMode =
  | { mode: 'cbr' }
  | { mode: 'vbr'; level: 1 | 2 | 3 | 4 | 5 }
  | { mode: 'cvbr' };

export interface EncoderSettingsV2 {
  encoderType: EncoderType;
  bitrateKbps: BitrateKbps;
  bitrateMode: BitrateMode;
  channels: 1 | 2;
  afterburner: boolean;
  customFfmpegPath?: string;
  threads: ThreadSetting;
}
```

### 3.2 Feature Flag Enablement
**File**: `src/ui/encoderPanel/featureFlags.ts`

```typescript
export const ENABLE_VBR = true;   // Was false
export const ENABLE_FDK = true;   // Was false
export const ENABLE_OPUS = true;  // NEW
```

### 3.3 Encoder Panel UI Changes
**File**: `src/ui/encoderPanel/` + `index.html`

**Basic Settings Section:**
- Encoder dropdown: Auto | FDK HE-AAC | Apple AAC | Native AAC | Opus
- Bitrate selector (existing, maybe expand range for Opus)
- Channels selector (existing)

**Advanced Settings Accordion:**
- VBR Level (1-5) - visible when FDK selected
- Afterburner toggle - visible when FDK selected
- Custom FFmpeg Path input - visible when FDK selected
- Bitrate mode indicator (read-only, shows CBR/VBR/CVBR based on encoder)

### 3.4 Encoder Availability Feedback
Show user which encoders are available:
- Green checkmark: Available on system
- Yellow warning: Requires custom FFmpeg path
- Tooltip explaining how to get FDK (build FFmpeg with --enable-libfdk-aac)

---

## Part 4: Implementation Order

### PR 1: Enhanced Encoder Engine

**Phase 1.1: Rust Encoder Types & Detection**
1. Update `EncoderType` enum (add Auto, FdkHeAac, Opus; rename HeAacV1→NativeAac)
2. Add `BitrateMode` enum (Cbr, Vbr, Cvbr)
3. Update `EncoderSettings` struct with new fields
4. Add `EncoderAvailability` struct and detection function
5. Update `validate_encoder_settings()` for new types

**Phase 1.2: Encoder Configuration Wiring**
1. Wire up FDK libfdk_aac: profile, vbr level, afterburner via FFmpeg options
2. Wire up Apple aac_at: CVBR mode configuration
3. Add Opus libopus: vbr, compression_level, application mode
4. Update `resolve_encoder()` for auto-detection fallback chain

**Phase 1.3: TypeScript Types & Bridge**
1. Update `src/types/encoder.ts` - new EncoderType, BitrateMode
2. Update `src/types/audio.ts` - boundary types
3. Update `toBoundaryEncoderSettings()` mapping
4. Update `src/lib/mocks.ts` for dev mode

**Phase 1.4: UI Updates**
1. Enable feature flags (VBR, FDK, OPUS)
2. Update `index.html` - encoder dropdown, VBR slider, afterburner toggle, custom path input
3. Update `encoderPanel/logic.ts` - read new settings
4. Add encoder availability indicator (checkmarks/warnings)
5. Conditional visibility (VBR/afterburner only for FDK)

**Phase 1.5: Testing & PR**
1. Unit tests for encoder detection
2. Run `scripts/quick-checks.sh`
3. Manual test with real files
4. Create PR: `feature/enhanced-encoder` → `new_encoder`

---

### PR 2: Adaptive Preview Enhancement

**Phase 2.1: Preview Config & Duration Options**
1. Add `min_segment_seconds` to `PreviewConfig`
2. Update `index.html` - preview dropdown with 15s, 45s, 60s options
3. Wire duration selection in `statusPanel/logic.ts`

**Phase 2.2: Frame Pipeline Refactor**
1. Add `PreviewState` struct for tracking per-file progress
2. Calculate `per_file_seconds` at preview start
3. Modify early-stop to be per-file, not total
4. Track file boundaries for chapter generation

**Phase 2.3: Chapter Generation**
1. Collect chapter markers during encoding
2. Generate FFMETADATA format in finalize stage
3. Embed chapters in preview output file

**Phase 2.4: Testing & PR**
1. Integration test for multi-file preview
2. Test duration options (15s, 45s, 60s)
3. Manual test chapter playback in preview
4. Create PR: `feature/adaptive-preview` → `new_encoder`

---

## Critical Files to Modify

### PR 1: Encoder Engine

**Rust:**
- `src-tauri/src/audio/settings_encoder.rs` - Types, validation, detection
- `src-tauri/src/audio/processor/encoder.rs` - Encoder configuration wiring
- `src-tauri/src/commands/audio.rs` - Command handler (if needed)

**TypeScript:**
- `src/types/encoder.ts` - UI types (EncoderType, BitrateMode)
- `src/types/audio.ts` - Boundary types
- `src/ui/encoderPanel/featureFlags.ts` - Enable VBR, FDK, OPUS flags
- `src/ui/encoderPanel/logic.ts` - Settings reading
- `src/ui/encoderPanel/dom.ts` - DOM cache for new elements
- `index.html` - Encoder dropdown, VBR slider, afterburner toggle, custom path
- `src/lib/mocks.ts` - Mock updates for dev mode

### PR 2: Adaptive Preview

**Rust:**
- `src-tauri/src/audio/context.rs` - PreviewConfig enhancement
- `src-tauri/src/audio/processor/frame_pipeline.rs` - Per-file tracking, early-stop logic
- `src-tauri/src/audio/processor/finalize.rs` - Chapter generation, FFMETADATA

**TypeScript:**
- `index.html` - Preview duration dropdown (15s, 45s, 60s)
- `src/ui/statusPanel/logic.ts` - Duration selection wiring

---

## Out of Scope (Future Work)
- Sample rate auto-selection based on bitrate
- HE-AAC v2 stereo-only warning UI
- Batch preview of multiple books
- Preview time selector redesign (keeping dropdown, just adding options)

---

## PR 1 Checklist: Enhanced Encoder
- [ ] All encoder types working (Auto, FDK, Apple, Native, Opus)
- [ ] Auto-detection selects best available (FDK > Apple > Native)
- [ ] Custom FFmpeg path works for FDK
- [ ] VBR levels 1-5 configurable for FDK
- [ ] Afterburner toggle works for FDK
- [ ] Apple aac_at uses CVBR mode
- [ ] Opus uses VBR with compression_level 10
- [ ] UI shows encoder availability (checkmarks/warnings)
- [ ] Conditional visibility (VBR/afterburner only for FDK)
- [ ] `scripts/quick-checks.sh` passes
- [ ] Manual test with real audiobook files

## PR 2 Checklist: Adaptive Preview
- [ ] Preview duration options work (15s, 45s, 60s)
- [ ] Default 30s still works when button pressed without selection
- [ ] Multi-file preview divides time across files
- [ ] Minimum segment floor (5s) respected
- [ ] Chapters embedded in preview file
- [ ] Chapter titles show source filenames
- [ ] Single file preview works (full duration to that file)
- [ ] `scripts/quick-checks.sh` passes
- [ ] Manual test chapter playback
