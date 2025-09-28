# Frontend & IPC Outline (TypeScript)

## Entry Point & IPC
- `src/main.ts`
  - Initializes panels (`fileImport`, `outputPanel`, `statusPanel`, `encoderPanel`, `coverArt`).
  - Exposes `window.testCommands` for manual testing (validate files/settings, process audiobook, metadata ops).
  - Advanced encoder panel init guarded (no-op if DOM missing).

## File & Settings Sources
- `src/ui/fileImport.ts` & `src/ui/fileList/*`
  - Manage drag/drop, list rendering, sorting.
  - `currentFileList` consumed by `StatusPanel` when launching processing.

- `src/ui/outputPanel.ts`
  - Maintains `OutputPanelState` (bitrate, sample rate, channels, output dir, filename pattern).
  - Hooks to UI controls; calculates derived output path (uses metadata + patterns).
  - Provides `getCurrentAudioSettings()` returning legacy `AudioSettings` (bitrate, channel config `'Mono'|'Stereo'`, sample rate `'auto'|{ explicit }`, output path string).

- Encoder panel
  - `src/ui/encoderPanel/*`
    - `featureFlags.ts` (currently disables VBR + FDK controls).
    - `dom.ts` caches advanced panel elements.
    - `logic.ts` disables placeholders when flags false; no state wiring yet.
  - `src/types/encoder.ts` (legacy planning doc) defines `EncoderSettingsV2` (flavor, profile, VBR placeholders, afterburner) – currently unused by `StatusPanel`, but informs future UI design.

## Type Contracts
- `src/types/audio.ts`
  - `AudioSettings`, `EncoderSettings` (mirrors Rust types: `encoderType`, `bitrateKbps`, `channels`, optional `aac_coder`, `afterburner`, `threads`).
  - `defaultEncoderSettings()` chooses `aac_at` on macOS else `he_aac_v1`.

- `src/types/events.ts`
  - Defines `EVENTS.PROGRESS`, `STAGES`, and `ProcessingProgressEvent` contract consumed across frontend.

## Status Panel Flow (`src/ui/statusPanel/logic.ts`)
1. On “Process” click → `startProcessing()`.
2. Validates `currentFileList` and `getCurrentAudioSettings()`; updates UI state.
3. Gathers metadata from form (via `getCurrentMetadata()` helper within file).
4. Builds v2 payload:
   ```ts
   const v2Payload = {
     inputFiles: validFilePaths,
     outputDir: (document.getElementById('output-dir-text') as HTMLInputElement)?.value ?? '',
     settings: (window as any).EncoderSettingsProvider?.() ?? {}
   };
   ```
   - Fallback: imports `defaultEncoderSettings()` if provider absent.
   - Coerces HE-AAC v2 to stereo (disables mono option in UI when triggered).
5. Invokes `process_audiobook_files_v2` with payload + metadata + optional preview seconds (`previewSeconds` argument or `ABB_PREVIEW_SECONDS`).
6. Starts progress listener: `listen(EVENTS.PROGRESS, ...)` updates UI, handles completion/cancellation.
7. Preview button triggers same flow with `previewSeconds: 30` and opens resulting file via `@tauri-apps/plugin-opener`.

## Event Handling & UI Updates
- Progress listener updates `ProcessingStatus` (stage, percentage, message) and toggles UI controls (cancel button state, progress bar, art thumbnail).
- `dom.ts` (StatusPanel) caches buttons, indicators, thumbnail elements.
- Cover art: `getCurrentCoverArt()` used to refresh progress card.

## Metadata & Output Coordination
- Metadata form (within OutputPanel/StatusPanel) feeds `getCurrentMetadata()` (StatusPanel) to send to backend.
- Output path preview uses sanitized metadata fields + user patterns (title/year vs author/title).

## Script Influence
- External script `shrink.sh` demonstrates toggles the UI may need:
  - Encoder selection (auto/fdk/apple) aligning with `EncoderSettings.encoderType`.
  - Quality controls (FDK VBR levels, Apple CVBR bitrate) → future UI sliders/dropdowns.
  - Channel override & thread counts → already partially wired via `EncoderSettings` fallback.
  - Preview/DRY-run toggles → inspiration for advanced debug UI or CLI integration.
  - Metadata + chapter handling → confirm planned UI for chapter import (currently not exposed).

## Future Engineering Tasks
1. Wire advanced encoder panel to provide `window.EncoderSettingsProvider` returning full `EncoderSettings` payload (including `aac_coder`, `threads`, future `afterburner`).
2. Add validation/UX to explain when options are ignored (e.g., afterburner without FDK, VBR toggles disabled).
3. Expose encoder availability feedback (detect `aac_at` / FDK via backend command) and surface in advanced panel.
4. Ensure output directory text box used for `ProcessV2Payload.outputDir` matches backend expectations (currently uses string from UI; needs normalization).
5. Expand progress UI for preview outputs (display preview file path, open button).
6. Synchronize types: update `src/types/encoder.ts` and `EncoderSettings` structures when backend evolves (avoid drift between TS and Rust).

