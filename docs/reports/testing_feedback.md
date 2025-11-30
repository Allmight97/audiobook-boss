# Dev feedback
1. I added several files and confirmed they loaded and I was able to change metadata. Input and metadata are fine as expected.
2. Audio processing is 100% broken for preview and process audiobook functions. When I press either button, the app flashes the progress bar as if it VERY quickly attempted to rpocess audio but stopped.
3. Most of the pills in the 'audio encoder settings' are non-functionaal: I'll list the status of each below:
  - ❌ Encoder: Defaults to auto, drop-down menur appears when clicked, all encoder options are greyed out except for 'auto'
  - ⚠️ Profile: Stays static (read-only) as intended but never changes because encoder options don't work. Double check the field will adapt.
  - ❌ Bitrate mode: Defaults to CBR on "auto" encoder, drop down works, all menu options greyed out save for "cbr"
  - ✅ Bitrate: drop down works, all menu options selectable.
  - ✅ Sample rate: defaults to 'auto' - all menu options selectable in drop down menu.
    - [ ] TODO #46 - add small text (under 'sample rate' field in 'Audio Encoder Settings' that shows what sample rate us being used in "auto" mode) - should read from input files and hardly ever need to be changed from source.
  - ✅ Defaults to 'auto', all menu items intact and selectable
    - Added to issue [#46](https://github.com/Allmight97/audiobook-boss/issues/46)
  Encoder options - literally does nothing (not even the checkbox works) and is stuck on 'Twoloop — Improves quality at low bitrates (always on)"

# Terminal output (from manual testing)
RUST_LOG=debug npm run tauri dev

> audiobook-boss@0.1.0 tauri
> tauri dev

     Running BeforeDevCommand (`npm run dev`)

> audiobook-boss@0.1.0 dev
> vite


  VITE v7.2.2  ready in 87 ms

  ➜  Local:   http://localhost:1420/
     Running DevCommand (`cargo  run --no-default-features --color always --`)
        Info Watching /Users/jstar/Projects/audiobook-boss/src-tauri for changes...
   Compiling audiobook-boss v0.1.0 (/Users/jstar/Projects/audiobook-boss/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.57s
     Running `target/debug/audiobook-boss`
[2025-11-30T03:24:54Z INFO  audiobook_boss_lib] Starting Audiobook Boss application
[2025-11-30T03:24:54Z INFO  audiobook_boss_lib::commands::audio] 🔍 list_available_encoders command invoked
[2025-11-30T03:24:54Z INFO  audiobook_boss_lib::audio::settings_encoder] 🔍 Detecting available encoders...
[2025-11-30T03:24:54Z DEBUG audiobook_boss_lib::audio::settings_encoder] 🔍 Encoder check 'libfdk_aac': FOUND
[2025-11-30T03:24:54Z DEBUG audiobook_boss_lib::audio::settings_encoder] 🔍 Encoder check 'aac_at': FOUND
[2025-11-30T03:24:54Z DEBUG audiobook_boss_lib::audio::settings_encoder] 🔍 Encoder check 'aac': FOUND
[2025-11-30T03:24:54Z INFO  audiobook_boss_lib::audio::settings_encoder] 🔍 Encoder detection results: fdk=true, aac_at=true, native_aac=true
[2025-11-30T03:24:54Z INFO  audiobook_boss_lib::commands::audio] 🔍 Returning encoder availability: EncoderAvailability { fdk_available: true, aac_at_available: true, native_aac_available: true }
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75271, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/02 - Foreward.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75146, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/04 - What I Know.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76901, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/03 - What I Do.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76274, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3` for reading
[2025-11-30T03:25:42Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:42Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75271, version: V3
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:42Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:42Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:42Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:42Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:25:45Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-11-30T03:25:45Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:25:45Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:25:45Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-11-30T03:25:45Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:25:45Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:25:45Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:25:45Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:25:45Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:26:28Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-11-30T03:26:28Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:26:28Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:26:28Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-11-30T03:26:28Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:26:28Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:26:28Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:26:28Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:26:28Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-11-30T03:26:30Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Projects/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-11-30T03:26:30Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-11-30T03:26:30Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-11-30T03:26:30Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-11-30T03:26:30Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-11-30T03:26:30Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-11-30T03:26:30Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-11-30T03:26:30Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-11-30T03:26:30Z DEBUG lofty::mpeg::properties] MPEG: VBR detected

---

# Agent Analysis (2025-11-30)

## Summary of Issues

| Priority | Issue | Status |
|----------|-------|--------|
| **P0 Critical** | Audio processing completely broken | Investigation needed |
| **P1 High** | Encoder dropdown options greyed out | Root cause identified |
| **P1 High** | Bitrate mode locked to CBR | Depends on encoder issue |
| **P1 High** | Encoder options section non-functional | Visibility logic issue |

---

## Issue 1: Audio Processing Broken (P0 Critical)

### Symptoms
- Progress bar flashes briefly then disappears
- No processing output in terminal logs
- Affects both Preview and Process Audiobook buttons

### Evidence
Terminal logs stop at file probing (lofty). No `process_audiobook_files_v2` command logs appear.

### Hypothesis
The command is either:
1. **Not being invoked** - Frontend error before `bridge.invoke()` call
2. **Failing immediately** - Rust command returns error before any processing logs
3. **Silently erroring** - Error caught in TS but not displayed to user

### Key Code Path
```
src/ui/statusPanel/logic.ts:165-303 (startProcessing)
  → bridge.invoke("process_audiobook_files_v2", payload)
  → src-tauri/src/commands/process.rs
```

### Investigation Steps for Next Session
1. **Check browser console** (Cmd+Option+I in the app) for JS errors
2. **Add debug logging** before `bridge.invoke()` call:
   ```typescript
   console.log("About to invoke process_audiobook_files_v2 with:", v2Payload);
   ```
3. **Check Rust command entry point** for early returns/panics

---

## Issue 2: Encoder Options Greyed Out (P1 High)

### Symptoms
- Only "Auto" is selectable in Encoder dropdown
- FDK AAC, Apple AAC, Native AAC all greyed out

### Evidence
Backend correctly detects ALL encoders:
```
🔍 Encoder detection results: fdk=true, aac_at=true, native_aac=true
🔍 Returning encoder availability: EncoderAvailability { fdk_available: true, aac_at_available: true, native_aac_available: true }
```

### Root Cause Analysis
The `disableDisallowedEncoders()` function in `logic.ts:354-387` checks:
- `fdk_he_aac`: disabled if `!ENABLE_FDK || !availability?.fdk_available`
- `aac_at`: disabled if `availability ? !availability.aac_at_available : false`
- `native_aac`: disabled if `availability ? !availability.native_aac_available : false`

**Possible causes:**
1. **Race condition**: `syncEncoderUI()` called before `cachedAvailability` is populated
2. **IPC data loss**: Availability object not received correctly from bridge
3. **TypeScript type mismatch**: Field names differ between Rust and TS

### Investigation Steps
1. **Open browser console** and check for `[EncoderPanel] Encoder availability:` log
   - If missing: `hydrateAvailability()` failed or bridge issue
   - If present but wrong: IPC serialization problem
2. **Add debug log to `disableDisallowedEncoders()`**:
   ```typescript
   debugLog("disableDisallowedEncoders:", { cachedAvailability, ENABLE_FDK });
   ```

---

## Issue 3: Bitrate Mode Locked to CBR (P1 High)

### Symptoms
- Only "CBR" is selectable
- VBR and CVBR greyed out

### Root Cause
This is a **downstream effect** of Issue 2. The `enforceBitrateModeCompatibility()` function locks bitrate mode based on effective encoder:
- `aac_at` → locks to CVBR
- `native_aac` → locks to CBR
- `fdk_he_aac` → locks to VBR

Since effective encoder is incorrectly resolving to `native_aac` (when availability is null/incorrect), it locks to CBR.

### Fix
Resolving Issue 2 will fix this automatically.

---

## Issue 4: Encoder Options Section (P1 High)

### Symptoms
- Shows "Twoloop — Improves quality at low bitrates (always on)"
- Checkbox is non-functional
- Should show Afterburner option for FDK

### Root Cause
The `syncEncoderOptions()` function toggles visibility based on effective encoder:
```typescript
dom.fdkOptions?.classList.toggle("hidden", encoder !== "fdk_he_aac");
dom.nativeOptions?.classList.toggle("hidden", encoder !== "native_aac");
dom.appleOptions?.classList.toggle("hidden", encoder !== "aac_at");
```

Since effective encoder is `native_aac` (due to Issue 2), native options are shown.

**Note**: The Twoloop checkbox has `disabled` attribute in HTML - this is intentional (always on).

### Fix
Resolving Issue 2 will fix this automatically.

---

## Recommended Next Steps (Priority Order)

### Immediate (P0)
1. **Debug processing flow**: Add logging to `startProcessing()` to identify where it fails
2. **Check browser console**: Look for JS errors when clicking Process/Preview

### High (P1)
3. **Debug encoder availability**: Add logging to `disableDisallowedEncoders()` to see what values it receives
4. **Verify IPC contract**: Ensure Rust `EncoderAvailability` matches TS type exactly

### Medium (P2)
5. **Add defensive initialization**: Call `syncEncoderUI()` again after a short delay as safety net
6. **Consider retry logic**: If availability fetch fails, retry once

---

## How to Get Browser Console Logs

When running `RUST_LOG=debug npm run tauri dev`:
1. The terminal shows Rust logs only
2. To see TypeScript/browser logs:
   - Right-click in app window → Inspect (or Cmd+Option+I)
   - Go to Console tab
   - Look for `[EncoderPanel]` prefixed logs and any red error messages

---

## Files to Investigate

| File | Relevance |
|------|-----------|
| `src/ui/encoderPanel/logic.ts` | Encoder UI logic, availability handling |
| `src/ui/statusPanel/logic.ts` | Processing initiation (startProcessing) |
| `src-tauri/src/commands/process.rs` | Backend processing command |
| `src-tauri/src/commands/audio.rs` | list_available_encoders command |

---

*Analysis by Claude agent - ready for next debug session*
