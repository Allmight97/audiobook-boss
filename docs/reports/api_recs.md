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
| `process_audiobook_files` | Active but superseded | `process_audiobook_files_v2` | Minimal mapping layer |
| `validate_audio_settings` | Active but limited | `validate_encoder_settings_cmd` | Significant feature mismatch |

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

### Migration Path Recommendations

**Phase 1: Documented Deprecation (Immediate)**
1. Update command documentation to mark v1 as deprecated
2. Add deprecation warnings in v1 command implementations
3. Update UI to show v2 as primary option

**Phase 2: Frontend Migration (Short-term)**
1. Migrate `src/ui/statusPanel/logic.ts` to exclusively use v2
2. Remove v1 references from UI components
3. Add v2 validation in encoder panel

**Phase 3: Code Cleanup (Medium-term)**
```rust
// Remove these from src-tauri/src/commands/audio.rs:
// - process_audiobook_files (v1)
// - validate_audio_settings (v1)
// - derive_v1_settings_from_v2 helper function
```

**Phase 4: Type Cleanup (Medium-term)**
1. Remove v1 types from `src/types/audio.ts`
2. Simplify boundary contracts
3. Update integration tests

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

### Immediate Actions

1. **Tag Legacy Code**
```rust
// Add to src-tauri/src/commands/audio.rs
#[deprecated(note = "Use process_audiobook_files_v2 instead")]
pub async fn process_audiobook_files(...) -> Result<ProcessCommandResult>
```

2. **Update Frontend Warnings**
```typescript
// Add to src/ui/statusPanel/logic.ts
console.warn("Using legacy v1 API - migrate to process_audiobook_files_v2");
```

### Medium-term Cleanup

1. ** consolidate v1/v2 mapping**
```rust
// Remove function and inline its logic into process_audiobook_files_v2
fn derive_v1_settings_from_v2(payload: &ProcessV2Payload) -> Result<AudioSettings>
```

2. **Remove v1 Types from Boundaries**
```typescript
// Remove from src/types/audio.ts
interface AudioSettings  // v1 only
export const AudioPresets = { ... }  // v1 only
```

### Long-term Improvements

1. **Simplify Pipeline Architecture**
2. **Unify Progress Reporting**
3. **Consolidate Validation Logic**

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
# Test both v1 and v2 endpoints
cargo test process_audiobook_files
cargo test process_audiobook_files_v2
```

### Migration Validation

1. **Parallel Testing**
   - Run v1 and v2 on same inputs
   - Compare output quality and performance
   - Verify feature parity

2. **Regression Testing**
   - Test all encoder settings combinations
   - Validate platform-specific behaviors
   - Check error handling consistency

---

## 7. Implementation Timeline

| Phase | Duration | Actions | Success Criteria |
|-------|----------|---------|------------------|
| Deprecation | 1 week | Add warnings, update docs | Warnings visible in logs |
| Frontend Migration | 2 weeks | Update UI to use v2 only; adopt `encoder-enhancement-plan.md` (v2-only); remove deprecated outline | No v1 calls in production; doc matches code |
| Code Cleanup | 1 week | Remove v1 implementations | v1 functions removed |
| Type Cleanup | 3 days | Remove v1 types | Cleaner boundary contracts |

---

## 8. Conclusion

The API documentation is in excellent shape with strong architecture and clear mapping to code location. The primary gaps are:
1. Incomplete documentation of the advanced v2 encoder features
2. Missing migration guidance from v1 to v2
3. Some UI integration details need documentation

The legacy v1 endpoints represent a maintenance burden and should be deprecated and removed following the phased approach outlined above. The v2 API provides significantly more capability and better alignment with modern audiobook processing needs.

By addressing these documentation gaps and executing the migration plan, the project will achieve:
- Clearer API boundaries
- Reduced maintenance burden
- Better feature discoverability
- Improved developer experience

---
