# Encoder v2 Integration — Audit and PR Plan

Last updated: 2025-08-22

## Summary

This report audits the planning docs in `docs/planning/new_encoder` against the current codebase and proposes a low‑risk PR sequence to introduce a v2 encoder with new frontend surfaces. The codebase today uses a single ffmpeg‑next engine with native AAC, no encoder selection, and no advanced options. The planning docs are largely coherent with this state. As a canonical decision, we adopt a multi‑PR strategy (see `README.md`) aligned with the expanded plan in `encoder_imp_plan.md`.

## Current State vs. Docs (highlights)

- Backend: single engine (`FfmpegNextProcessor`), selects `ff::codec::Id::AAC`; applies `strict=experimental` and `aac_coder=twoloop` unless disabled via `ABB_DISABLE_TWOOLOOP`. No HE‑AAC v1/v2 profiles, no `aac_at`, no threads, no afterburner mapping.
- Types: only `AudioSettings { bitrate, channels, sample_rate, output_path }` across IPC and pipeline.
- Frontend: output panel controls for bitrate/channels/sample rate; no advanced encoder UI; payload is `AudioSettings`.
- Tests: strong coverage for metadata/cover art and path validation; gaps for encoder options/validation.
- Docs: accurately call out missing `EncoderSettings` and mapping layers; good enumeration of impact zones and test needs.

## Coherence Findings

- Accurate: dependency mapping of surfaces (frontend types/UI, Tauri commands, processor split, metadata bridge), and gaps for `EncoderSettings`, profile/threads mapping, AAC‑AT selection.
- Feasible: Type additions are localized; encoder mapping can be contained within `processor/encoder.rs`; validation can live in a new `audio/settings_encoder.rs`.
- Logging: current logs summarize `encoder_setup` using `AudioSettings`; docs’ plan to add INFO/DEBUG logs for resolved encoder params is straightforward once `EncoderSettings` exist.

## Inconsistencies and Fixes

- Link fixes (applied):
  - `ISSUE_DRAFT_outcome2_advanced_encoder_ui.md` now points to `docs/planning/new_encoder/*` and test gaps to `docs/planning/new_encoder/encoding_test_gaps.md`.
  - `outcome2_advanced_encoder_plan.md` now references `docs/planning/new_encoder/*` and uses full paths for external API docs in `docs/external-apis/*`.
- Strategy alignment (resolved):
  - We choose the multi‑PR path with minimal behavior change early, then backend mapping, then UI. The plan in `outcome2_advanced_encoder_plan.md` reflects this.

## Recommended PR Strategy (safe, incremental)

1) PR: Contracts + Validation (no behavior change)
   - Add `src-tauri/src/audio/settings_encoder.rs` with `EncoderType`, `AacCoder`, `ThreadSetting`, `EncoderSettings` (+ serde/defaults, macOS default `AacAt`).
   - Add validation helpers: bitrate whitelist for v2; HE‑AAC v2 stereo enforcement; threads bounds.
   - Optional: tauri `validate_encoder_settings` for early UI checks.

2) PR: Encoder Mapping (backend only)
   - Add encoder selection: `aac_at` on macOS (find‑by‑name via FFI if needed), else native `aac`.
   - Map profile (HE/HEv2), `aac_coder` (twoloop/fast), `threads` (auto/off/fixed). Best‑effort with robust logs.
   - INFO summary line with resolved params; DEBUG for each applied/ignored option.

3) PR: Frontend UI + Wiring
   - Add TS `EncoderSettings` types and an `EncoderPanel` (or extend output panel) with encoder/coder/afterburner/threads.
   - Disable unsupported controls on AAC‑AT; lock `channels=2` for HE‑AAC v2; guard numeric threads.
   - Update payload from UI to include `EncoderSettings`.

4) PR: Tests
## Branching & Merge Targets

- One PR per phase (PR 1–4/5) to keep scope and review small.
- Base branch: `feat/new_encoder` as the integration branch. Avoid merging intermediate phases directly to `main`.
- For each phase: create a short-lived branch (e.g., `feat/new_encoder-phase1-types`), open a PR targeting `feat/new_encoder`, require green `cargo test`, `cargo test --features safe-ffmpeg`, and `cargo clippy -- -D warnings`.
- After all phases merge into `feat/new_encoder` and stabilize, open a final PR from `feat/new_encoder` → `main`.
- Small fixes or regressions discovered during later phases should be PRs against `feat/new_encoder`.

   - Unit: validation errors (HEv2+mono, threads), serde round‑trip, option mapping success/fallback behavior.
   - Integration: AAC‑AT selection on macOS, HE‑AAC v1/v2 profile open + stereo lock, logs contain summary.

## Single‑PR Alternative (faster, higher risk)

- In‑place upgrade of existing command/types; update all frontend call sites atomically; add minimal tests to cover new behavior. Requires short merge freeze to avoid payload drift.

## Risks & Dependencies

- AAC‑AT selection: may require FFI helper to find encoder by name; acceptable (precedent in metadata FFI usage).
- Afterburner: only for `libfdk_aac`; treat as no‑op with INFO “ignored” unless FDK is compiled in (future feature flag).
- Backward compatibility: avoid breaking existing UI by staging types/validation before backend mapping and UI wiring.
- Cross‑platform: gate AAC‑AT with `#[cfg(target_os = "macos")]`, log fallback elsewhere.

## Next Steps

- See `README.md` for canonical decisions and links. Proceed with PR 1 (contracts + validation); keep behavior stable until PR 2. Add tiny audio fixtures for fast tests and set up macOS‑only tests where needed.

---

Prepared by: new encoder audit (docs vs. code)
Scope analyzed: `src-tauri/src/audio/*`, `src/ui/*`, `src/types/*`, `docs/planning/new_encoder/*`, `docs/external-apis/*`.

