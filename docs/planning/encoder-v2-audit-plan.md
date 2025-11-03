# Encoder v2 & UI Alignment Audit (2025-??)

**Note**: This document reflects the state prior to PR D completion. As of Nov 2025, v1 commands have been removed. Encoder setup consumes v2 `EncoderSettings` directly; command handler retains minimal v2→v1 mapping for legacy validation paths only.

## 1. Repository State Review

### Backend pipeline (Rust)
- `src-tauri/src/commands/audio.rs::process_audiobook_files_v2` retains minimal v2→v1 mapping for legacy validation paths (technical debt). Encoder settings are passed via `ProcessingContext.encoder_settings_v2`.
- `src-tauri/src/audio/processor/encoder.rs` consumes `ProcessingContext.encoder_settings_v2` directly for encoder configuration (profile flags, `aac_coder`, thread hints, bitrate, channels, encoder selection).
- `src-tauri/src/audio/settings_encoder.rs` owns validation, availability checks, and encoder name resolution; UI-constrained options must match the enums/whitelists exposed here.
- Discovery helpers (e.g., `is_encoder_available_by_name`) are synchronous and expect ffmpeg to be initialized. There is no dedicated Tauri command exposing availability yet.

### Frontend surfaces (TypeScript)
- `src/types/audio.ts::EncoderSettings` mirrors the Rust shape (encoderType, bitrateKbps, channels, optional aacCoder/afterburner/thread settings). `defaultEncoderSettings()` chooses AAC-AT on macOS, HE-AAC v1 elsewhere.
- `src/ui/statusPanel/logic.ts` invokes `process_audiobook_files_v2`, using `toBoundaryEncoderSettings` adapter to normalize UI types to boundary shape.
- `src/types/encoder.ts` defines `EncoderSettingsV2` (UI-only) and `toBoundaryEncoderSettings` adapter; boundary type is `src/types/audio.ts::EncoderSettings`.
- `src/ui/encoderPanel/*` only disables hidden knobs via feature flags; it does **not** read/write the shared `EncoderSettings` payload.

### Documentation baseline
- `docs/planning/encoder-enhancement-plan.md` proposes several UI/backend changes but still references `MediaProcessingPlan`, `ProcessRequestV2`, and other shapes that diverge from the current code. It also suggests skip heuristics and plan-only flows without defining validation/UX behavior.
- External API docs (`docs/external-apis/ffmpeg-next.md`, `lofty.md`) remain relevant for ffmpeg-next patterns and metadata handling; they do not cover new command surfaces such as encoder discovery.

## 2. Identified Risks, Questions, & Second-Order Effects

1. **Type divergence (`EncoderSettingsV2` vs `EncoderSettings`)** — Maintaining two incompatible front-end type definitions invites bugs where the UI serializes a shape the backend rejects. Second-order effect: contract drift would break `scripts/ensure-contract.sh` and any tests that deserialize `EncoderSettings` from localStorage or fixtures.
2. **Encoder availability feedback loop** — Without a command exposing `is_encoder_available_by_name`, the UI cannot surface AAC-AT/FDK availability. Adding such a command means touching `src-tauri/src/commands/audio.rs`, which impacts the contract-checking script and requires new tests to guard the JSON shape. Second-order: availability polling must be throttled to avoid repeated ffmpeg initialization (potential performance hit on launch).
3. **Plan-only / preview UX** — The existing command already accepts `preview_seconds` but the UI lacks toggles. If we extend the payload with `plan_only` or skip heuristics, we must reconcile progress events (`processing-progress`) to avoid confusing percentages. Second-order: preview outputs are named `.preview.m4b`; introducing plan-only results requires new Tauri responses and probably UI modals/log surfaces.
4. **Skip-optimized heuristic reliability** — The plan’s bitrate thresholds rely on `AudioFile.bitrate`, which may be `None` for VBR sources. A naive implementation could skip high-quality files incorrectly or reprocess everything. Need a defensive strategy (e.g., require both bitrate + codec detection, fall back to processing) and user messaging.
5. **Threads setting interplay** — Rust validation allows `threads` up to 1024, but the UI currently has no control. Exposing sliders/spinners without guardrails could lead to unrealistic concurrency requests, affecting CPU utilization and stability.

## 3. Outstanding Questions for Repository Owner

- **Encoder priorities:** Should AAC-AT remain the default when both AAC-AT and libfdk_aac are present, or should users explicitly opt into FDK when discovered?
- **FDK sourcing policy:** Do we support user-provided path overrides for libfdk_aac (as hinted by `externalFfmpegPath` in `EncoderSettingsV2`), or is detection limited to ffmpeg’s compiled-in encoders?
- **Plan-only UX:** What is the desired output? A structured report in the UI, a downloadable file, or console logs? Clarifying this shapes both the command response and frontend rendering.
- **Skip heuristic transparency:** How should the UI communicate when files are skipped? Toast notifications, inline badges per file, or summary counts?

## 4. Proposed Work Plan (sequenced)

1. **Unify front-end encoder types & state plumbing**
   - Remove/retire `src/types/encoder.ts` in favor of `src/types/audio.ts::EncoderSettings`.
   - Wire `src/ui/encoderPanel` inputs to load/save through `EncoderSettings` (including localStorage persistence via `state.ts`).
   - Ensure `StatusPanel` invokes the command with the updated state.
   - **Tests:** Vitest unit tests for state serialization + defaults; end-to-end DOM test for panel enabling/disabling.
   - **2nd-order considerations:** Update any scripts or fixtures referencing the old type to prevent runtime serialization issues.

2. **Expose encoder availability & UI gating**
   - Add a `list_available_encoders` Tauri command returning `{ aacAt: boolean; libfdkAac: boolean }`.
   - Cache the availability result on the frontend to avoid repeated calls (e.g., lazy fetch during panel init with memoization).
   - Disable or annotate unavailable encoder options in the dropdown with status text.
   - **Tests:** Rust unit test asserting JSON response when encoders are absent (mock via feature gating or environment); contract test verifying command is registered; Vitest DOM test ensuring options are disabled when availability is false.
   - **Side effects:** Update `scripts/ensure-contract.sh` expectations; consider storing availability in global state if other panels need it.

3. **Extend command payload for plan-only + preview controls**
   - Confirm desired UX with owner, then extend `ProcessV2Payload` (or create a wrapper) to carry `plan_only` and `skip_optimized` flags.
   - Update `ProcessingContext` to branch on these flags (plan-only should bypass heavy encoding and return a structured summary; skip should adjust progress totals to reflect ignored files).
   - Frontend: add toggles + tooltips, ensure progress UI handles plan-only (likely show modal instead of progress stream).
   - **Tests:** Rust integration test covering plan-only response (no output file, summary matches inputs); test skip logic on mixed-bitrate fixtures; TypeScript unit tests for UI state toggles and plan summary rendering.
   - **Second-order:** Progress emitter may need new stage values (e.g., `Planning`). Ensure `processing-progress` contracts remain backward-compatible or add optional fields with defaults.

4. **Implement skip-optimized heuristic safely**
   - Refine detection to require codec & bitrate (e.g., skip only when codec already AAC-LC and bitrate ≤ target minus margin).
   - Document fallback behavior when metadata is missing; log decisions for telemetry/debug.
   - Frontend: surface summary counts and allow users to override (e.g., “Process anyway”).
   - **Tests:** Unit tests for heuristic with synthetic `AudioFile` data; integration test confirming skipped files reduce encoder invocations but still report completion.
   - **Second-order:** Ensure skipping does not break output ordering or metadata merges; adjust progress totals accordingly.

5. **Thread setting UI & validation loop**
   - Add UI control (select or numeric input) bound to `ThreadSetting` with safe presets (Auto, Off, 2, 4, 8).
   - Update validation messaging if user selects values outside supported range.
   - **Tests:** Rust unit tests for validation errors; Vitest tests for UI serialization.
   - **Second-order:** Evaluate interactions with plan-only (should still report chosen threads) and ensure defaults remain platform-appropriate.

6. **Documentation & tooling updates**
   - Refresh `docs/planning/encoder-enhancement-plan.md` to align terminology (replace `ProcessRequestV2`, `MediaProcessingPlan` references).
   - Extend `docs/external-apis/ffmpeg-next.md` with availability command usage patterns and plan-only flow constraints.
   - Update `docs/external-apis/tauri-commands.md` with the new command signature and payload fields.
   - **Tests/Checks:** Run `scripts/quick-checks.sh` after implementation; ensure doc changes match reality to prevent future drift.

## 5. Testing Strategy Summary

- **Rust:** unit tests in `settings_encoder.rs` (threads/availability), new tests for skip heuristics and plan-only summary; async integration tests in `src-tauri/tests` covering command invocations.
- **TypeScript:** Vitest suites for encoder panel state management, availability gating, and command payload assembly; potential Playwright smoke test once UI toggles exist.
- **Manual:** macOS-focused validation of AAC-AT vs native fallback, verifying logs and progress events for plan-only/skip flows.

## 6. Next Steps

- Address outstanding questions with the repo owner to confirm priorities (AAC-AT vs FDK defaults, plan-only UX expectations).
- Sequence work items to land incrementally (types + UI wiring → availability command → payload extensions → heuristics/threads).
- Schedule contract & regression tests after each milestone to maintain pipeline stability during the v2 migration.
