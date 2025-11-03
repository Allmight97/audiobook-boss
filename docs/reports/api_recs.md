# API Documentation Audit & Recommendations

*Date: 2025-10-11 (updated)*
*Scope: Tauri commands, events, and frontend API surfaces*

---

## Executive Summary

The external API documentation in `docs/external-apis/` is substantially accurate and well-maintained, capturing the core API surfaces effectively. However, several gaps were identified between the documentation and current implementation, particularly around the v2 encoder API and some UI integration details that have evolved since the documentation was written. This update reflects recent documentation cleanup (removed `docs/specs/coding_guidelines.md`, removed `docs/specs/development.md`, archived `docs/specs/db.json`) and re-points recommendations to living sources of truth.

This report provides a comprehensive comparison between documented APIs and the actual implementation, with specific focus on the legacy v1 to v2 transition and recommendations for resolving legacy components.

---

## 1. Documentation vs Audit Alignment

### ✅ Well-Documented Areas

**Tauri Commands (`tauri-commands.md`)**
- All 12 commands are documented and correctly mapped to their Rust implementations
- UI consumers are properly identified for each command
- Versioning (v1/v2) is accurately reflected
- Testing harness (`window.testCommands`) is properly documented

**Event System (`tauri-patterns.md`, `tauri-ts-boundaries.md`)**
- `processing-progress` event contract is well-documented
- Throttling behavior (200ms) is accurately documented
- Lifecycle management (listen/unlisten) patterns are correct and comprehensive

**FFmpeg Integration (`ffmpeg-next.md`)**
- Timestamp/PTS handling patterns precisely match implementation
- Encoder setup details are accurate with correct av_opt_set usage
- Progress calculation formulas align perfectly with code

**Path Handling (`path-handling.md`)**
- Validation allowlist (mp3, m4a, m4b, aac, wav, flac) matches implementation
- Symlink handling is correctly documented
- Atomic rename behavior on macOS is accurately described

---

## 2. Legacy v1 vs v2 API Analysis

### Current State of v1 (Legacy) Endpoints

| v1 Endpoint | Status | v2 Equivalent | Complexity Gap |
|------------|--------|---------------|-----------------|
| `process_audiobook_files` | Removed | `process_audiobook_files_v2` | N/A |
| `validate_audio_settings` | Removed | `validate_encoder_settings_cmd` | N/A |

**Current State**: v1 endpoints have been removed from the IPC surface. The codebase is v2-only at the command boundary. Encoder setup consumes v2 `EncoderSettings` directly; command handler retains minimal v2→v1 mapping for legacy validation paths only (technical debt).

Legacy findings below are retained for historical context prior to removal.

**Key Issues with v1 Legacy:**
1. **Limited Encoder Control**: v1 only supports basic bitrate and channel config
2. **No Thread Control**: v1 cannot manage encoder threading behavior
3. **Missing Platform Detection**: v1 doesn't leverage platform-specific encoder capabilities
4. **Simplified Settings**: No support for AAC coder selection or afterburner optimization

### v2 Encoder Settings Advantages

The v2 API provides significant enhancements:

```typescript
// v1 limitations
interface AudioSettings {
  bitrate: number;                    // Simple integer
  channels: ChannelConfig;            // Only mono/stereo
  sampleRate: SampleRateConfig;       // Auto or explicit
  outputPath: string;                  // Fixed path only
}

// v2 advantages
interface EncoderSettings {
  encoderType: EncoderType;           // Platform-aware selection
  bitrateKbps: 56 | 64 | 72 | ...;    // Constrained bitrate options
  channels: 1 | 2;                    // Integer channels
  aacCoder?: AacCoder;               // twoloop/fast options
  afterburner?: boolean;              // Quality optimization
  threads: ThreadSetting;            // auto/off/fixed control
}
```

**V2 Benefits:**
- Platform-aware encoder selection (`aac_at` on macOS, native FFmpeg elsewhere)
- Thread management for performance optimization
- Quality controls (AAC coder, afterburner)
- Constrained, validation-friendly bitrate options

**Migration Status**: Complete. v1 endpoints removed. UI uses v2 exclusively via `process_audiobook_files_v2` and `validate_encoder_settings_cmd`.

---

## 3. Identified Documentation Gaps

### Missing v2 Encoder Details

**Gaps in `tauri-commands.md`:**
- `ThreadSetting` type variants (`auto`, `off`, `fixed {value}`) not documented
- Platform-aware default selection logic missing
- AAC coder options twoloop/fast not detailed
- Afterburner flag behavior undocumented

**Recommendation:**
```markdown
### Advanced Encoder Settings (v2)
- encoderType: Platform-aware ('aac_at' on macOS, 'native_aac' elsewhere)
- threads: 
  - auto: Let encoder decide (default)
  - off: Single-threaded encoding
  - fixed: Manual thread count specification
- aacCoder: twoloop (better quality) vs fast (faster encoding)
- afterburner: Enable quality optimizations where supported
```

### Progress Event Stage Mapping Incomplete

**Current Documentation:**
- Mentions progress percentage ranges vaguely
- Missing specific stage-to-percentage mapping

**Actual Implementation:**
```rust
// From src-tauri/src/audio/constants.rs
const PROGRESS_ANALYZING_START: f32 = 0.0;
const PROGRESS_ANALYZING_END: f32 = 10.0;
const PROGRESS_CONVERTING_START: f32 = 10.0;
const PROGRESS_CONVERTING_MAX: f32 = 80.0;
const PROGRESS_METADATA_START: f32 = 80.0;
const PROGRESS_FINALIZING: f32 = 95.0;
const PROGRESS_CLEANUP: f32 = 98.0;
const PROGRESS_COMPLETE: f32 = 100.0;
```

### UI Integration Gaps

**Undocumented Surfaces:**
1. **Encoder Panel Structure** (`src/ui/encoderPanel/`)
   - Feature flag management
   - Platform-specific UI variations
   - Settings validation flow

2. **Cover Art Integration** (`src/ui/coverArt.ts`)
   - Load flow through `load_cover_art_file` command
   - Image format validation
   - Base64 conversion for display

3. **Output Path Management**
   - Directory picker integration
   - Preview path derivation logic
   - v2 payload construction

---

## 4. Specific Documentation Update Recommendations

### Priority 1: Critical Gaps

1. **Expand `tauri-commands.md`**
   - Add complete v2 encoder settings structure
   - Document platform-aware default selection
   - Include validation examples for each setting

2. **Update `tauri-patterns.md`**
   - Add complete stage percentage mapping table
   - Document progress event payload fields
   - Include cancellation event handling details

3. **Create v2 Migration Guide**
   - Step-by-step frontend migration
   - Breaking changes documentation
   - Timeline for v1 deprecation

### Priority 2: Enhanced Documentation

1. **UI Integration Documentation** (point to concise planning docs rather than large audits)
  - Encoder panel and IPC (v2-only) → `docs/planning/encoder-enhancement-plan.md`
  - Cover art loading and management → `src/ui/coverArt.ts`
  - Output path and preview generation → `src/ui/outputPanel.ts`

2. **Platform-Specific Behavior**
   - macOS AAC encoder detection
   - Thread configuration differences
   - Performance tuning guidelines

### Priority 3: Supporting Documentation

1. **Contract Testing Guide**
   - Boundary contract verification steps
   - Event contract validation
   - TypeScript type safety checks

2. **API Evolution Policy**
   - Versioning strategy
   - Backward compatibility rules
   - Deprecation process

---

## 5. Code Cleanup Recommendations

### Current State

**v1 Removal**: Complete. v1 commands removed from IPC surface. v1 types (`AudioSettings`) remain only for internal legacy validation paths in command handler (technical debt).

**Remaining Work**:
1. Remove v2→v1 mapping from command handler (future PR)
2. Consider removing v1 types from boundaries once mapping is removed
3. Simplify pipeline architecture
4. Unify progress reporting
5. Consolidate validation logic

---

## 6. Testing & QA Recommendations

### Contract Verification

1. **Event Contract Tests**
```bash
# Verify progress event structure
RUST_LOG=debug npm run tauri dev
# Process sample and verify stage transitions
```

2. **Boundary Type Tests**
```bash
# TypeScript contract verification
tsc --noEmit
# Verify no any types in boundary interfaces
```

3. **Integration Tests**
```bash
# Test v2 endpoint
cargo test process_audiobook_files_v2
```

### Validation

1. **Regression Testing**
   - Test all encoder settings combinations
   - Validate platform-specific behaviors
   - Check error handling consistency
   - Verify adapter handles UI types correctly

---

## 8. Conclusion

The API documentation is in excellent shape with strong architecture and clear mapping to code location. The v1→v2 migration is complete; v1 endpoints have been removed from the IPC surface. The primary remaining gaps are:
1. Incomplete documentation of the advanced v2 encoder features
2. Some UI integration details need documentation

The codebase now uses v2 exclusively at the command boundary, providing:
- Clearer API boundaries
- Reduced maintenance burden
- Better feature discoverability
- Improved developer experience

---
