# GitHub Issues Audit - 2025-11-30

## Summary

14 open issues analyzed and categorized. Added `bug` labels to 4 unlabeled bugs (#33, #34, #35, #38).

---

## SECURITY BUGS (Critical - Work These First)

| # | Title | Impact |
|---|-------|--------|
| **#33** | Metadata commands skip path validation | **HIGH** - Local file disclosure/tampering via traversal paths. Violates repo security policy. |
| **#34** | Cover-art loader bypasses path validation | **HIGH** - Arbitrary file exfiltration via IPC. Same root cause as #33. |

Both share the same fix pattern: add `validate_input_audio_path()` to command handlers. Could be tackled in a single PR.

---

## BUGS (Standard Priority)

| # | Title | Impact |
|---|-------|--------|
| **#38** | Series/Book # inputs never reach metadata payload | **MEDIUM** - Data loss. User-entered series info not written to M4B. TSOA preview is misleading. |
| **#35** | Cover art optimization ignores EXIF orientation | **MEDIUM** - Mobile photos display sideways in output files. |
| **#47** | Preview file auto-open fails (Tauri opener) | **LOW** - Cosmetic. File still works, just won't auto-open. |

---

## FEATURES / TODOs

| # | Title | Priority |
|---|-------|----------|
| **#46** | Visible sample rate/channel info below dropdowns | Low - UX polish |
| **#40** | Investigate Opus encoder support | Blocked - Waiting on ABS container story |

---

## REFACTORING / TECH DEBT

| # | Title | Priority |
|---|-------|----------|
| **#45** | Remove redundant `vbr` property in EncoderSettingsV2 | Low - Type cleanup |
| **#37** | Replace `window.EncoderSettingsProvider` with events | Low - Maintainability |
| **#32** | Simplify `load_cover_art_file` validation | Low - Code dedup |

---

## PERFORMANCE / HARDENING

| # | Title | Priority |
|---|-------|----------|
| **#44** | Encoder panel event cleanup + DOM caching | Medium - Memory leak prevention |
| **#42** | Adaptive preview hardening & tests | Medium - Test coverage |
| **#31** | Integer arithmetic in cover blending | Low - Micro-optimization |

---

## Recommended Work Order

1. **#33 + #34** (Security) - Single PR to add path validation to all metadata/cover commands
2. **#38** (Bug) - Series metadata not reaching payload
3. **#35** (Bug) - EXIF orientation
4. **#44** (Perf) - Event listener cleanup / memory leaks
5. Rest as time permits
