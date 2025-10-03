# Encoder Panel & Pipeline Enhancements Plan

## Scope & Alignment
- **Goal**: lift proven behaviors from `scripts/shrink.sh` into the Tauri app while keeping a single `ffmpeg-next` engine.
- **Assumed UI knobs** (confirm with repository owner before implementation):
  - Encoder selection (`auto | aac_at | libfdk_aac`) with availability hints.
  - Bitrate / quality presets (CVBR + HE profiles + future VBR toggles).
  - Preview toggle (30 s) and dry-run/log preview.
  - “Skip already optimized” option.
  - Merge mode choice (auto | flatten | separate) with chapter metadata preview.
  - Advanced diagnostics (write ffmpeg command, chapter order) gated behind debug mode.
- **Surfaces**: `src/types/encoder.ts`, `src/ui/encoderPanel/*`, `src/ui/fileList/*`, `src-tauri/src/commands/audio.rs`, `src-tauri/src/audio/processor/*`, `src-tauri/tests/*`.

## Baseline Readings
- `src/ui/encoderPanel/logic.ts` currently feature-gates VBR/FDK controls, providing scaffolding for availability-driven toggles.
- `src/types/encoder.ts` models `EncoderFlavor`, reserved VBR fields, and defaults to `aac_at` on macOS.
- `src-tauri/src/audio/settings_encoder.rs` validates encoder settings and probes encoder availability (`is_encoder_available_by_name`).
- `src-tauri/src/audio/processor/encoder.rs` maps v2 settings to ffmpeg-next encoder options, logging ignored fields.

## Feature Plan

### 1. Encoder Discovery & UI Feedback
- **Backend**: expose encoder availability via a lightweight command.

```rust
// ... existing code ...
#[derive(Debug, Serialize)]
pub struct EncoderAvailability {
    pub aac_at: bool,
    pub libfdk_aac: bool,
}

#[tauri::command]
pub async fn list_available_encoders() -> Result<EncoderAvailability> {
    Ok(EncoderAvailability {
        aac_at: is_encoder_available_by_name("aac_at"),
        libfdk_aac: is_encoder_available_by_name("libfdk_aac"),
    })
}
// ... existing code ...
```

- **Frontend**: query availability during panel init; disable unavailable options and show guidance.

```startLine:endLine:src/ui/encoderPanel/logic.ts
// ... existing code ...
const availability = await invoke<EncoderAvailability>('list_available_encoders');
if (!availability.libfdk_aac) {
  if (dom.encoderSelect) {
    const opt = dom.encoderSelect.querySelector("option[value='external_fdk']");
    if (opt) opt.disabled = true;
  }
  if (dom.fdkStatus) dom.fdkStatus.textContent = 'FDK unavailable – install ffmpeg with libfdk_aac';
}
if (!availability.aac_at && dom.encoderSelect) {
  const opt = dom.encoderSelect.querySelector("option[value='aac_at']");
  if (opt) opt.disabled = true;
}
// ... existing code ...
```

### 2. Bitrate & Quality Presets (CVBR / HE Profiles / Future VBR)
- **Types**: surface curated presets using the backend whitelist.

```startLine:endLine:src/types/encoder.ts
export const BITRATE_PRESETS: Array<{ label: string; value: EncoderSettingsV2['bitrateKbps'] }> = [
  { label: 'Speech • 64 kbps', value: 64 },
  { label: 'Narration • 72 kbps', value: 72 },
  { label: 'Balanced • 80 kbps', value: 80 },
  { label: 'Rich • 88 kbps', value: 88 },
  { label: 'Max • 96 kbps', value: 96 },
];
```

- **UI**: bind select/slider to update `EncoderSettingsV2.bitrateKbps`; update profile hints (LC vs HE) to match script-driven quality levels.

### 3. Preview & Dry-Run Workflow
- **Command Input**: extend v2 process payload.

```rust
// ... existing code ...
#[derive(Debug, Deserialize)]
pub struct ProcessRequestV2 {
    // existing fields
    #[serde(default)]
    pub preview_seconds: Option<u16>,
    #[serde(default)]
    pub plan_only: bool,
}
// ... existing code ...
```

- **Pipeline**: if `plan_only`, short-circuit after building `MediaProcessingPlan` and return a plan summary (input count, encoder choice, expected runtime).
- **Frontend**: add toggles mirroring `PREVIEW` / `DRY` script flags; show plan modal or console summary.

### 4. Skip Already Optimized Files
- **Backend**: add guard in prepare stage.

```rust
// ... existing code ...
fn should_skip_optimized(entry: &AudioFile, target_channels: u8) -> bool {
    let br_kbps = entry.bit_rate_kbps().unwrap_or(0);
    match target_channels {
        1 => br_kbps > 0 && br_kbps <= 64,
        2 => br_kbps > 0 && br_kbps <= 80,
        _ => false,
    }
}
// ... existing code ...
if request.skip_optimized && should_skip_optimized(file, plan.settings.channels.channel_count()) {
    log::info!("skip=optimized file={}", file.path.display());
    continue;
}
```

- **UI**: checkbox with tooltip explaining heuristic sourced from `shrink.sh`.

### 5. Merge Modes & Chapter Metadata Preview
- **Frontend**: add merge mode selector with preview of resulting chapter order.
- **Backend**: extend `MediaProcessingPlan` to carry merge strategy; generate ffmetadata chapters before encoding (similar to script logic).

### 6. Metadata Inspection Panel
- **Frontend**: reuse `analyze_audio_files` results to show duration, sample rate, bitrate, codec in the file list.
- **Backend**: ensure analyzer returns structured numeric fields (currently stored in `AudioFile`).

### 7. Concurrency Controls
- **UI**: expose thread setting options tied to `ThreadSetting` (Auto, Single, Fixed N).
- **Backend**: plumb through existing `EncoderSettings.threads`; ensure validation errors surface clearly.

### 8. Debug Artifacts (Opt-in)
- **Backend**: enrich plan-only response with ffmpeg command, chapter order, and output paths.
- **Frontend**: add “Export debug plan” button (debug builds only) to write artifacts via Tauri FS API.

## Testing Strategy
- Rust: extend `settings_encoder.rs` tests for new request fields and skip logic; add integration tests for preview/plan-only flows (`src-tauri/tests/preview_30s_integration.rs`).
- TypeScript: add encoder panel state tests (availability gating, bitrate presets) using Vitest.
- Manual: run `ABB_DISABLE_FASTPATH=1 RUST_LOG=debug npm run tauri dev`; verify UI toggles and log output.

## Open Questions
1. Confirm which UI knobs ship in the first iteration vs backlog.
2. Decide whether FDK exposure is informational (status only) or functional (enable encode path).
3. Define where debug exports should land (temp directory vs user choice).

## Validation Checklist
- `cargo fmt --all -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `tsc --noEmit`
- `npm run build`


