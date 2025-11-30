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

---

# Agent Investigation Session 2 (2025-11-30)

## Key Finding 1: Browser Console Required

The terminal only shows **Rust logs**. All frontend debug logs (`[EncoderPanel]` prefixed) go to the **browser console** (Cmd+Option+I). The existing analysis correctly notes this but it's critical - we need browser console output to diagnose frontend issues.

## Key Finding 2: Encoder Disabling Logic Analysis

Reviewed `disableDisallowedEncoders()` in `logic.ts:354-387`. The logic is **correct**:

```typescript
case "aac_at":
  option.disabled = availability ? !availability.aac_at_available : false;
```

With `availability = { aac_at_available: true }`, this evaluates to `!true = false` (enabled).

**Root Cause Hypothesis**: There's a timing or state issue - either:
1. `syncEncoderUI()` being called before `cachedAvailability` is populated
2. Bridge invoke not returning data to frontend correctly
3. Some other code re-disabling options after initialization

## Key Finding 3: Processing Failure Evidence

Terminal shows **NO** `encoder_v2 summary:` log from `process_audiobook_files_v2`. This log appears at line 91 of `commands/audio.rs` and should fire immediately upon command invocation. Its absence means:

- **The Rust command is never invoked** - failure is in frontend before `bridge.invoke()`
- This is a **frontend error**, not a backend processing issue

## Key Finding 4: Suspected Failure Point

In `startProcessing()` (statusPanel/logic.ts:165-303), the payload construction relies on:
1. `EncoderSettingsProvider` function returning valid DOM-read settings
2. `toBoundaryEncoderSettings()` converting UI state to boundary type
3. `outputDir` being populated from `#output-dir-text` input

If any of these throw or return invalid data, processing fails silently (caught in generic catch block).

---

## Recommended Debug Actions

| Priority | Action | What to Look For |
|----------|--------|------------------|
| **1** | Open browser console (Cmd+Opt+I) | Red errors, `[EncoderPanel]` logs |
| **2** | Add `console.log` before bridge.invoke | Verify payload shape |
| **3** | Check `#output-dir-text` value | Empty output path = likely failure |
| **4** | Verify `EncoderSettingsProvider()` works | Call in console: `window.EncoderSettingsProvider()` |

---

## Files Reviewed

| File | Relevance |
|------|-----------|
| `src/ui/encoderPanel/logic.ts` | Encoder UI sync logic |
| `src/ui/encoderPanel/dom.ts` | DOM element cache |
| `src/ui/encoderPanel/featureFlags.ts` | ENABLE_FDK = true (correct) |
| `src/ui/statusPanel/logic.ts` | Processing initiation |
| `src/types/encoder.ts` | Boundary conversion logic |
| `src-tauri/src/commands/audio.rs` | Backend command |

---

*Investigation by Claude agent - awaiting browser console output for definitive diagnosis*

---

# Browser DevTools Debug Session (2025-11-30)

## Test Environment
- `npm run dev` → http://localhost:1420/
- Chrome DevTools connected
- Browser mode (mocks active, not Tauri)

## Key Findings

### 1. Frontend Logic is CORRECT
When mock returns `fdk_available: true`, all encoder options enable correctly:
```json
{
  "options": [
    {"value": "auto", "disabled": false},
    {"value": "fdk_he_aac", "disabled": false},
    {"value": "aac_at", "disabled": false},
    {"value": "native_aac", "disabled": false}
  ],
  "hint": "FDK detected ✓",
  "bitrateMode": "vbr",
  "profile": "HE-AAC v1"
}
```

### 2. Processing Flow Works
- Files validated correctly (2 files, 35 MB combined)
- `EncoderSettingsProvider()` returns valid settings
- Processing fails with expected error: "Output directory not selected"
- **Not a frontend bug** - validation is working

### 3. Mock Was Incorrect
Original mock returned `fdk_available: false`, which caused FDK option to be disabled. Fixed in `src/lib/mocks.ts`.

### 4. Console Log Flow (Healthy)
```
[EncoderPanel] Initializing encoder panel...
[Bridge Mock] Invoke: list_available_encoders
[EncoderPanel] Encoder availability: {fdk_available: true, ...}
[EncoderPanel] Encoder panel ready
```

## Root Cause Hypothesis for Tauri Mode

Since frontend logic works correctly in browser mode, the issue in Tauri mode is likely:

1. **IPC Timing Issue**: The `list_available_encoders` invoke may be returning before the Rust side is fully initialized
2. **Serialization Mismatch**: Field names in Rust `EncoderAvailability` may not match TS expectations
3. **Race Condition**: Some other code path calling `syncEncoderUI()` before availability is populated

## Recommended Next Steps

1. **Run `RUST_LOG=debug npm run tauri dev`** and open browser console (Cmd+Opt+I in app window)
2. Look for `[EncoderPanel] Encoder availability:` log - check if object has correct field values
3. If availability shows all true but options still disabled, add debug log to `disableDisallowedEncoders()`
4. Check if there's a second call to `syncEncoderUI()` after the first one

## Mock Fix Applied
```typescript
// src/lib/mocks.ts - line 88
fdk_available: true,  // was: false
```

---

*Browser debug session by Claude agent*

---

# ROOT CAUSE FOUND & FIXED (2025-11-30)

## The Bug: Field Name Mismatch (snake_case vs camelCase)

**Rust/Tauri sends (camelCase):**
```json
{
  "fdkAvailable": true,
  "aacAtAvailable": true,
  "nativeAacAvailable": true
}
```

**TypeScript expected (snake_case):**
```typescript
type EncoderAvailability = {
  fdk_available: boolean;      // undefined!
  aac_at_available: boolean;   // undefined!
  native_aac_available: boolean; // undefined!
};
```

Result: `availability.fdk_available` was `undefined`, and `!undefined` = `true` (disabled).

## Fix Applied

Updated `src/ui/encoderPanel/logic.ts` to use camelCase field names:
- `fdk_available` → `fdkAvailable`
- `aac_at_available` → `aacAtAvailable`
- `native_aac_available` → `nativeAacAvailable`

Also updated `src/lib/mocks.ts` for consistency.

## Why This Happened

Tauri uses serde for serialization. By default, serde converts Rust snake_case field names to JavaScript camelCase. The TypeScript type wasn't updated to match.

## Verification

- TypeScript compiles: `npx tsc --noEmit` ✅
- Reload Tauri app to test encoder dropdowns

---

*Root cause identified and fixed by Claude agent*

---

# Second Serialization Fix: BitrateMode (2025-11-30)

## The Bug

Processing failed with: `missing field 'value'`

**TypeScript sent:**
```json
{ "mode": "vbr", "level": 3 }
```

**Rust expected (due to `#[serde(content = "value")]`):**
```json
{ "mode": "vbr", "value": 3 }
```

## Fix Applied

Updated `src/types/audio.ts` and all usages in `src/types/encoder.ts` and `src/ui/encoderPanel/logic.ts`:
- `{ mode: "vbr", level: N }` → `{ mode: "vbr", value: N }`

## Lesson Learned

Rust serde attributes control JSON field names:
- `#[serde(rename_all = "camelCase")]` → converts struct fields to camelCase
- `#[serde(tag = "mode", content = "value")]` → for enums, uses "mode" as discriminant and "value" for payload

TypeScript types must match these serialization conventions exactly.

---

*Second serialization fix by Claude agent*

---

# Third Serialization Fix: ChannelConfig (2025-11-30)

## The Bug

**TypeScript sent** (numbers for mono/stereo):
```typescript
export type EncoderChannelConfig = "auto" | 1 | 2;
```

**Rust expected** (strings):
```rust
#[serde(rename_all = "snake_case")]
pub enum ChannelConfig { Auto, Mono, Stereo }
// Serializes as: "auto", "mono", "stereo"
```

Would have failed when user selected Mono or Stereo from dropdown.

## Fix Applied

Updated `src/types/audio.ts`:
```typescript
export type EncoderChannelConfig = "auto" | "mono" | "stereo";
```

Also updated all usages in:
- `src/ui/encoderPanel/logic.ts` (DOM value conversion)
- `src/ui/statusPanel/logic.ts` (legacy settings mapping)
- `src/types/encoder.ts` (validation and sanitization)

---

*Third serialization fix by Claude agent*
