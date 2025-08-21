# Outcome 1 — 30s Preview with Cover Art (macOS‑first)

Context anchors:
- Single‑engine ffmpeg‑next only; no shell FFmpeg.
- Keep fast‑path disabled by default until validated (ABB_DISABLE_FASTPATH=1).
- Avoid adding new code to `audio/media_pipeline.rs`; use `audio/processor/*` façade modules.
- Include cover art in preview with minimal KISS adjustments.

## Goals
- Add a fixed 30‑second preview mode that stops encoding early and writes a valid `.m4b`.
- Include cover art and basic metadata as in full encodes.
- Wire the existing UI Preview control to call the backend and open the produced file.

## Scope
- Backend: Tauri command, processing context, frame pipeline early‑stop, finalize with cover art.
- Frontend: Wire the existing “30s Preview” placeholder to invoke preview, disable controls during run, and show result.
- macOS‑first; cross‑platform later.

## Design

### API surface (Tauri)
- Extend existing processing command request payload with an optional `previewSeconds: number`.
  - Fixed UI value for now: `30`.
  - Keep env fallback for dev: `ABB_PREVIEW_SECONDS` if payload field is absent.
- Response: include `previewFilePath?: string` when preview was requested/successful.

### Processing context
- Add `preview_seconds: Option<f64>` to the context (`audio/context.rs` or `audio/session.rs`).
- Thread it through to the frame pipeline (`audio/processor/frame_pipeline.rs`).

### Frame pipeline early‑stop
- In `process_decoded_frames` (or equivalent), after updating `running_pts` per decoded/encoded progress, compute seconds as `running_pts as f64 / target_sample_rate as f64`.
- If `Some(preview_seconds)` and `elapsed >= preview_seconds`:
  - Emit INFO log with `elapsed` and target.
  - Transition to finalize: flush encoder (send `None`), write trailer.

### Output path policy
- Derive from final path: `<final_basename>.preview.m4b` in the same output directory (or session temp subdir if already set).
- Overwrite policy: always overwrite any existing preview file of the same name to keep UX simple.

### Cover art and metadata
- Use the same cover art orchestration as full encodes:
  - Pre‑header native cover art stream addition (if any).
  - Post‑header fallback via Lofty (writer) in finalize.
- Do not skip cover art or basic tags in preview mode; follow the same code path for correctness.

### Progress & events
- Keep emitting progress events as usual (duration will plateau at ~30s in preview mode).
- Add a single INFO log “preview early‑stop reached” with elapsed seconds; do not change event shapes.

### Error handling
- Return `Result<T, AppError>` everywhere; no `unwrap/expect`.
- On failure to write the preview file, propagate an error that can be surfaced to the UI.

## Data contracts (authoritative)

### TypeScript (frontend payload/response)
```ts
export interface PreviewRequest {
  previewSeconds?: number; // if absent, backend may read ABB_PREVIEW_SECONDS
}

export interface ProcessCommandPayload /* existing shape */ {
  // ... existing fields ...
  previewSeconds?: number;
}

export interface ProcessCommandResult /* existing shape */ {
  // ... existing fields ...
  previewFilePath?: string; // present only for preview runs
}
```

### Rust (backend config)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
  pub seconds: f64,
}

#[derive(Debug, Clone)]
pub struct ProcessingContext {
  // ... existing fields ...
  pub preview: Option<PreviewConfig>,
}
```

Notes:
- Prefer payload `previewSeconds` over env; env is a dev convenience.
- Response includes `previewFilePath` only when preview mode triggers.

## Constraints and behaviors
- If total input duration < 30s, preview stops at EOF (valid shorter preview).
- Early‑stop threshold measures encoded timeline using `running_pts / target_sample_rate`.
- Path security: use the same validation and output probing as full encodes.
- Idempotency: preview file is overwritten on re‑runs.
- Cancellation: existing cancellation logic (if any) still applies; early‑stop is not cancellation.

## Backend mapping (how preview integrates)
- `src-tauri/src/commands/audio.rs`
  - Extend command payload to read `previewSeconds` (and optional `ABB_PREVIEW_SECONDS`).
  - Normalize into `PreviewConfig` and attach to `ProcessingContext`.
  - Return `previewFilePath` in the success response when applicable.
- `src-tauri/src/audio/session.rs` or `src-tauri/src/audio/context.rs`
  - Add `preview: Option<PreviewConfig>` to the context struct; plumb through constructors/builders.
- `src-tauri/src/audio/processor/frame_pipeline.rs`
  - Update the main loop to compute `elapsed_seconds` and early‑stop when `Some(preview)` and `elapsed >= preview.seconds`.
  - On early‑stop: send encoder flush, break loop, proceed to finalize.
- `src-tauri/src/audio/processor/finalize.rs`
  - Reuse finalize logic; derive preview output path `<final_basename>.preview.m4b` and overwrite.
  - Keep cover art orchestration identical to full flow (native first, Lofty fallback).
- Logging
  - INFO once: resolved preview seconds, output path.
  - INFO once on early‑stop: elapsed seconds.

## UI behavior rules
- Preview button sends `{ previewSeconds: 30 }` and disables other processing controls while running.
- Show “Previewing…” state with progress bar; on success, reveal path and an “Open file” action.
- If the preview is shorter because inputs are short, show the actual duration in the completion message.
- If an error occurs (e.g., write failure), surface it inline and restore controls.

## Atomized implementation checklist

### Backend (Rust)
1) Add `PreviewConfig` and `preview: Option<PreviewConfig>` to processing context.
2) Extend Tauri command payload parsing to accept `previewSeconds` and env fallback.
3) Thread preview through prepare/execute into frame pipeline.
4) Implement early‑stop in `frame_pipeline.rs` using `running_pts / target_rate`.
5) Ensure encoder flush and trailer write on early‑stop.
6) Implement preview output naming and overwrite policy in finalize.
7) Include cover art via existing pre/post header paths.
8) Return `previewFilePath` in command response when previewing.
9) Add INFO logs for resolved preview seconds and early‑stop event.

### Frontend (TypeScript)
1) Update command types to include `previewSeconds?: number` and `previewFilePath?: string`.
2) Wire the “30s Preview” button to send `{ previewSeconds: 30 }`.
3) Disable other controls during preview; show “Previewing…”.
4) On success, display preview path and enable an “Open file” action (opener plugin).
5) Handle errors and restore UI state.

## Validation plan
- Manual test: select one or more input files; run preview; verify `.preview.m4b` exists, opens, and includes cover art.
- Duration check: confirm ~30s (±1s) unless inputs shorter; verify logs show early‑stop.
- Re‑run preview to confirm overwrite behavior.

## Acceptance criteria
- Produces a valid `.m4b` preview around 30 seconds with cover art when provided.
- Normal full encodes unaffected when preview is not requested.
- Response includes `previewFilePath` for preview runs; UI can open the file.
- Logs show resolved preview seconds and early‑stop; no encoder contract violations.

## Tests
- Integration: preview produces `.preview.m4b` around 30s; embeds cover art when provided; idempotent overwrite.
- Smoke: full encode path with no preview remains functional.

## Risks & mitigations
- Duration drift at frame boundaries → Accept ±1s; still flush and write trailer.
- File overwrite ambiguity → Always overwrite the preview file; log action.
- Platform differences → macOS‑first scope keeps risk contained.

## Operational notes
- Keep `ABB_DISABLE_FASTPATH=1` during preview trials.
- Use `RUST_LOG=info|debug` to observe early‑stop and finalize logs.
