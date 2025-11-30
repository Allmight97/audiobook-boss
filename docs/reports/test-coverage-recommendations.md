# Test Coverage Recommendations

**Date**: 2024-11-29
**Context**: Post PR #41 (Adaptive Preview Enhancement) review
**Related Issue**: #42 (PR2 Follow-up: Adaptive Preview Hardening & Test Coverage)

## Executive Summary

This report identifies test coverage gaps across three critical areas:
1. Preview function (adaptive multi-file preview)
2. Main audiobook processing function
3. Encoder/profile UI functions

Current coverage is strong for unit-level validation but lacks integration and end-to-end tests for the processing pipeline.

---

## 1. Preview Function Test Gaps

### Current Coverage
- `PreviewConfig::per_file_seconds()` - 6 unit tests in `context.rs`
- `sanitize_chapter_title()` - 6 unit tests in `frame_pipeline.rs`
- `PreviewState` struct methods - basic tests in `frame_pipeline.rs`

### Gaps Identified

#### 1.1 Integration Tests (HIGH PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Multi-file preview generation | Process 3+ files, verify excerpts from each | `tests/preview_integration.rs` |
| Floor enforcement | 7 files × 30s request → verify 5s minimum applied | `tests/preview_integration.rs` |
| Short file handling | File shorter than per-file allocation | `tests/preview_integration.rs` |
| Single file regression | Verify single-file preview still works | `tests/preview_integration.rs` |

#### 1.2 Unit Tests (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| `PreviewAction` transitions | Verify `NextFile` → `StopAll` state machine | `frame_pipeline.rs` |
| `check_per_file_preview_stop()` | Edge cases: zero files, negative elapsed | `frame_pipeline.rs` |
| Chapter marker timestamp accuracy | PTS to millisecond conversion | `frame_pipeline.rs` |

#### 1.3 Boundary Tests (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Preview duration options | 15s, 30s, 45s, 60s all produce output | `tests/preview_integration.rs` |
| `ABB_PREVIEW_SECONDS` env var | Override via environment | `tests/preview_integration.rs` |

---

## 2. Main Processing Function Test Gaps

### Current Coverage
- Path validation - comprehensive tests in `path_validation.rs`
- Encoder settings validation - thorough tests in `settings_encoder.rs`
- Bitrate/thread/channel validation - unit tests pass

### Gaps Identified

#### 2.1 Integration Tests (HIGH PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| End-to-end M4B generation | Process sample files → verify output exists and is valid | `tests/processing_integration.rs` |
| Multi-file merge | Verify files concatenate in correct order | `tests/processing_integration.rs` |
| Progress event emission | Verify `processing-progress` events fire correctly | `tests/processing_integration.rs` |
| Cancellation handling | Verify partial cleanup on cancel | `tests/processing_integration.rs` |

#### 2.2 Encoder Pipeline Tests (HIGH PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Encoder resolution | `Auto` → actual encoder based on availability | `tests/encoder_integration.rs` |
| FDK encoder path | When libfdk_aac available, verify VBR encoding | `tests/encoder_integration.rs` |
| aac_at encoder path | macOS AudioToolbox encoder with CVBR | `tests/encoder_integration.rs` |
| Native AAC fallback | When others unavailable, verify CBR encoding | `tests/encoder_integration.rs` |
| Afterburner flag | FDK-only, ignored for others | `settings_encoder.rs` |

#### 2.3 Sample Rate / Channel Tests (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Sample rate Auto detection | Verify first-file sample rate used | `tests/processing_integration.rs` |
| Channel downmix | Stereo → Mono conversion | `tests/processing_integration.rs` |
| Channel Auto mode | Preserve source channels | `tests/processing_integration.rs` |

#### 2.4 Metadata Integration (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Metadata embedding | Title, author, narrator in output | `tests/metadata_integration.rs` |
| Cover art embedding | JPEG/PNG cover in M4B | `tests/metadata_integration.rs` |
| Metadata validation warnings | Compatibility warnings logged | `tests/metadata_integration.rs` |

---

## 3. Encoder/Profile UI Functions Test Gaps

### Current Coverage
- TypeScript types defined in `src/types/encoder.ts`
- `toBoundaryEncoderSettings()` - no tests
- `defaultBitrateModeFor()` - no tests
- `EncoderSettingsProvider` - no tests

### Gaps Identified

#### 3.1 TypeScript Unit Tests (HIGH PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| `toBoundaryEncoderSettings()` | All encoder types produce valid boundary format | `src/types/encoder.test.ts` |
| `defaultBitrateModeFor()` | Auto→VBR, FDK→VBR, AacAt→CVBR, Native→CBR | `src/types/encoder.test.ts` |
| Bitrate validation | Valid range 48-128 kbps | `src/types/encoder.test.ts` |
| Thread setting serialization | Auto, Off, Fixed(n) serialize correctly | `src/types/encoder.test.ts` |

#### 3.2 Encoder Panel Logic Tests (HIGH PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| `disableDisallowedEncoders()` | Options disabled when availability false | `src/ui/encoderPanel/logic.test.ts` |
| `updateBitrateModeForEncoder()` | Mode changes when encoder type changes | `src/ui/encoderPanel/logic.test.ts` |
| Availability caching | `cachedAvailability` populated from backend | `src/ui/encoderPanel/logic.test.ts` |
| UI state → settings conversion | DOM values → `EncoderSettings` object | `src/ui/encoderPanel/logic.test.ts` |

#### 3.3 IPC Contract Tests (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| `list_available_encoders` response | Matches `EncoderAvailability` type | `src/lib/bridge.test.ts` |
| `validate_encoder_settings_cmd` | Valid settings pass, invalid rejected | `src/lib/bridge.test.ts` |
| `process_audiobook_files_v2` payload | `ProcessV2Payload` serializes correctly | `src/lib/bridge.test.ts` |

#### 3.4 Profile Panel Tests (MEDIUM PRIORITY)
| Test Case | Description | File Location |
|-----------|-------------|---------------|
| Profile selection | Audiobook preset loads defaults | `src/ui/profilePanel/logic.test.ts` |
| Custom settings override | User changes persist | `src/ui/profilePanel/logic.test.ts` |
| Output path construction | Directory + filename + extension | `src/ui/profilePanel/logic.test.ts` |

---

## 4. Priority Matrix

### Must Have (Before Next Release)
1. **Encoder detection integration test** - Verify FFI calls return expected results
2. **End-to-end M4B generation test** - Process real audio → valid output
3. **`toBoundaryEncoderSettings()` unit tests** - Contract between UI and backend
4. **`disableDisallowedEncoders()` tests** - UI reflects actual availability

### Should Have (Next Sprint)
1. Multi-file preview integration test
2. Progress event emission verification
3. Metadata embedding tests
4. IPC contract tests

### Nice to Have (Backlog)
1. Cancellation stress tests
2. Large file handling tests
3. Edge case filename sanitization
4. Performance regression tests

---

## 5. Test Infrastructure Recommendations

### Rust Integration Tests
```bash
# Create test fixture directory
mkdir -p src-tauri/tests/fixtures

# Add sample audio files (short, ~5s each)
# - mono.mp3, stereo.mp3, 44100hz.mp3, 48000hz.mp3
```

### TypeScript Test Setup
Current setup in `src/test/setup.ts` provides Tauri API mocks. Extend with:

```typescript
// src/test/mocks/encoder.ts
export const mockEncoderAvailability = {
  fdkAvailable: true,
  aacAtAvailable: true,
  nativeAacAvailable: true,
};

export const mockEncoderAvailabilityNoneAvailable = {
  fdkAvailable: false,
  aacAtAvailable: false,
  nativeAacAvailable: false,
};
```

### CI Integration
Add to GitHub Actions workflow:
```yaml
- name: Run Rust integration tests
  run: cargo test --test '*_integration' -- --test-threads=1

- name: Run TypeScript tests with coverage
  run: npm run test:coverage
```

---

## 6. Immediate Action Items

Based on current regression investigation:

1. **Add encoder detection test** (`settings_encoder.rs`)
   ```rust
   #[test]
   fn test_encoder_detection_returns_at_least_one() {
       let avail = detect_available_encoders();
       assert!(
           avail.fdk_available || avail.aac_at_available || avail.native_aac_available,
           "At least one encoder must be available"
       );
   }
   ```
   *(Note: This test already exists but could be strengthened)*

2. **Add FFI probe test** (`settings_encoder.rs`)
   ```rust
   #[test]
   fn test_native_aac_always_available() {
       // Native AAC should always be available in any FFmpeg build
       assert!(is_encoder_available_by_name("aac"), "Native AAC encoder not found");
   }
   ```

3. **Add TypeScript encoder type tests** (`src/types/encoder.test.ts`)
   - Verify `toBoundaryEncoderSettings()` produces valid JSON
   - Verify `defaultBitrateModeFor()` returns correct modes

---

## References

- Issue #42: PR2 Follow-up tracking
- `AGENTS.md`: Testing & Verification section
- `scripts/coverage.sh`: Coverage report generation
- `src/test/setup.ts`: Existing Tauri mock infrastructure
