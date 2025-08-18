# Media Pipeline Refactor — Single Source of Truth (Merged Plan)

Context anchors:
- Single-engine `ffmpeg-next` only; shell FFmpeg was removed [[memory:6466795]].
- Avoid adding new code to `media_pipeline.rs` beyond targeted fixes until it’s broken up [[memory:6473122]].
- Runs succeed with fast-path disabled; failures were seen with fast-path enabled due to suspected encoder frame contract violations [[memory:6473122]].

## Current state (audited)
- Encoding pipeline still lives in `audio/media_pipeline.rs`: `create_audio_encoder`, `setup_encoder`, `encode_and_write_frame`, `finalize_encoding`, plus decode/resample/accumulator loop and cover-art pre/post-header.
- `audio/buffer.rs` provides `SampleAccumulator` that clamps and sanitizes samples; `encode_and_write_frame()` also performs unconditional sanitize (duplicate work).
- Orchestrator split already exists (`audio/processor/{prepare,execute,finalize,selection,mod}.rs`).
- Native cover art embedding is attempted; finalize stage verifies/embeds via Lofty as fallback.
- AAC twoloop enhancement and `strict=experimental` are attempted via `av_opt_set` with graceful fallback if unavailable.

## Decisions:
- Keep fast-path disabled by default (docs/CI) until contract validation and long-run stability are proven. Retain the `ABB_DISABLE_FASTPATH` toggle.
- Centralize sanitation in `audio/buffer.rs` only; remove encode-layer sanitize after validator lands.
- Introduce a debug-only frame contract validator guarding encoder sends (no runtime cost in release).
- Split `media_pipeline.rs` into cohesive modules; keep the façade tiny. No behavior changes except explicitly called out below.

## Target module layout (post-split)
- `audio/processor/encoder.rs`
  - `create_audio_encoder`
  - `setup_encoder` (container, stream, metadata pre/post‑header orchestration)
  - `encode_and_write_frame`
  - `finalize_encoding`
  - Constraints: functions ≤55 LOC; module ≤400 LOC; return `Result<T, AppError>`; no `unwrap/expect`.
- `audio/processor/streams.rs`
  - `setup_decoder_and_resampler` (logs input/output formats; returns decoder, resampler, stream index)
- `audio/processor/frame_pipeline.rs`
  - `process_input_packets`
  - `process_decoded_frames` (fast-path env-guarded; accumulator used; progress emits consistent)
  - `process_input_file`
- `audio/processor/cover_art.rs`
  - Pre-header cover-art stream add; post-header cover-art packet write; logs decisions; uses existing `metadata` helpers
- Optional: `audio/media_plan.rs` (move `MediaProcessingPlan` + duration helper out of `media_pipeline.rs`)
- `audio/media_pipeline.rs` becomes a façade wiring the above, keeping context structs minimal.

## Phases, scope, and acceptance

### P0 Guardrails (no feature work)
1) Centralize sanitization
   - Action: delete per-encode sanitize in `encode_and_write_frame()` once validator is in place and accumulator is confirmed as the single sanitation point.
   - Acceptance: no encoder NaN/Inf errors; waveforms differ only by clamping; tests green.
2) Debug-only frame contract validator
   - `cfg(debug_assertions)` assertion before `send_frame`:
     - buffers allocated; planar format matches encoder; `nb_samples > 0` and `≤ frame_size` (except EOF/flush semantics);
     - PTS monotonic; all samples finite and within [-1,1].
   - Acceptance: violations fail fast in debug; no overhead in release.
3) Fast-path policy
   - Keep `ABB_DISABLE_FASTPATH=1` default in docs/CI until P1 completes; retain toggle afterwards.

### P1 Module split (behavior-preserving)
4) Extract encoder concerns to `audio/processor/encoder.rs`
   - Move: `create_audio_encoder`, `setup_encoder`, `encode_and_write_frame`, `finalize_encoding`.
   - Acceptance: compile + tests; logs unchanged; `media_pipeline.rs` shrinks.
5) Extract decoder+resampler setup to `audio/processor/streams.rs`
   - Move: `setup_decoder_and_resampler`.
   - Acceptance: same IO format logs; identical behavior.
6) Extract frame pipeline to `audio/processor/frame_pipeline.rs`
   - Move: `process_input_packets`, `process_decoded_frames`, `process_input_file`.
   - Acceptance: progress events unchanged; accumulator behavior unchanged; compile + tests.
7) Extract cover-art orchestration to `audio/processor/cover_art.rs`
   - Acceptance: native embedding still attempted; finalize fallback preserved; cover-art tests green.
8) Keep `media_pipeline.rs` ≤250 LOC as façade.

### P1.5 Settings regression fix (“settings not honored”)
9) Verify UI → encoder param mapping
   - Log resolved params at start: `target_sample_rate`, `target_channels`, `bitrate`, UI `settings` snapshot.
   - Ensure `SampleRateConfig::Explicit` is respected through decoder→resampler→encoder.
   - Acceptance: explicit selections reflected in output; add a simple integration test.

### P2 Encoder enhancements (feature‑flagged, graceful fallback)
10) Threading
   - Prefer encoder context APIs; else attempt `av_opt_set("threads", N)`; log effective thread mode.
11) Twoloop
   - Keep `aac_coder=twoloop` best‑effort; respect `ABB_DISABLE_TWOOLOOP=1`.
12) Optional AAC VBR plumbing (backend only for now)
   - Extend `AudioSettings` with `bitrate_mode: Cbr(u32) | Vbr(u8)`.
   - If VBR: set `bit_rate=0`, set quality via `global_quality`/`AV_CODEC_FLAG_QSCALE` (or `av_opt_set("q", ...)`); log mode; fallback to CBR.
   - Acceptance: env `ABB_AUDIO_MODE=VBR3` selects VBR without UI; logs confirm; fallback cleanly handled.

### P3 Observability, tests, and perf
13) Logging
   - Reduce per‑packet INFO to DEBUG; keep periodic INFO every N packets; retain high‑signal warnings.
14) Tests
   - Unit: accumulator behavior; validator trip cases; encoder selection/fallbacks.
   - Integration: output duration delta vs sum(input) < 0.1%; cover art present when provided.
15) Metrics
   - Record baseline throughput and durations in `docs/reports/audio_baseline_metrics.md`.

### P4 Re‑enable fast‑path (conditional)
16) Only after contract tests + long‑run stability passes; keep rollback toggle.

## FFmpeg/ffmpeg‑next validation notes
- Planar f32 (`ffmpeg_next::format::Sample::F32(planar)`) is supported and aligns with accumulator.
- AAC `frame_size`: typically 1024 (LC). Some builds may report 0 (variable). Accumulator falls back to 1024 to maintain contract.
- `aac_coder=twoloop`: valid on native AAC; treat failures as non‑fatal and continue with AAC‑LC.
- `strict=experimental`: may be ignored on modern builds; attempting via `av_opt_set` is safe with non‑fatal fallback.
- Threading: prefer official encoder context APIs; if absent, `av_opt_set("threads", ...)` is acceptable with logging and fallback.

## Doc alignment: Lofty and Tauri 2
- Lofty (0.20):
  - For MP4/M4B, ensure a `TagType::Mp4Ilst` tag exists before writing and use `TaggedFileExt::save_to_path` with default options. Cover art should be inserted via `Tag::push_picture(Picture)` with an appropriate `MimeType` (JPEG/PNG). This matches current `writer.rs` behavior and is consistent with modern Lofty docs.
  - When native cover art embedding is skipped/fails, fallback with Lofty is appropriate. Dimension and size heuristics in validation are advisory and non‑blocking, which aligns with best practices.
  - Keep metadata writes idempotent and avoid clearing container‑specific atoms beyond tag fields we control.
- Tauri 2:
  - Use `tauri::Window.emit` (Emitter) for progress events as implemented in `progress::reporter.rs`; no legacy Tauri 1 APIs are present.
  - Capabilities/permissions are configured in `capabilities/default.json` (Tauri 2 pattern). Refactor does not require changes, but ensure any new commands remain within declared capabilities.
  - Maintain event payloads as serializable structs (`ProgressEvent`) and avoid large binary payloads over IPC.

## Risks and mitigations
- Hidden coupling during extraction → small, sequenced moves; compile+tests between steps.
- FFmpeg build variability → all encoder/quality enhancements are best‑effort with clean fallbacks.
- Fast‑path instability → keep disabled by default until validated by contract tests and long‑runs.

## Rollback toggles
- `ABB_DISABLE_FASTPATH=1`
- `ABB_DISABLE_TWOOLOOP=1`
- `ABB_AUDIO_MODE=VBR{1..5}` (temporary, backend‑only during P2)

## Next steps (update this section after each phase)
- Execute P0 (centralize sanitize, add validator, keep fast‑path off by default in docs/CI).
- Perform P1 extraction in small edits with green builds between each.
- Address settings regression (P1.5) immediately after split.
