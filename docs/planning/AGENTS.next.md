# AGENTS.md (Proposed Revision)

This file is a candidate replacement for the root `AGENTS.md`. It keeps `AGENTS.md` as the single source of truth for agent guidance and references only the minimum necessary docs. Do not modify the root file until this proposal is approved.

## What to do before editing
- Validate your plan with the repository owner before implementing non-trivial changes (prefer smallest safe diffs).
- Quick checks (must pass before and after changes):
  - Rust (from `src-tauri/`):
    - `cargo test`
    - `cargo clippy -- -D warnings`
    - `cargo fmt --all -- --check`
  - Frontend (from repo root):
    - `tsc --noEmit`
    - `npm run build` (or `npm run dev` during iteration)

## Current goals (Q4 2025)
- Branch: `new_encoder`.
- Goals:
  1) Synthesize lessons from `scripts/shrink.sh` into ffmpeg-next encoder and expose settings in UI.
  2) Audit bugs/features/TODOs and keep a single backlog.
  3) Define “good coverage” and align tests with current architecture.

## Hard guardrails
- Single engine: ffmpeg-next only. No shell ffmpeg anywhere in app code paths.
- Path security: validate all inputs via `audio::path_validation::validate_input_audio_path()`.
- Progress system: backend emits `processing-progress`; UI must display percentage, message, `current_file`, and `eta_seconds`.
- Metadata: Lofty for read/write with internal `AudiobookMetadata`.
- Avoid adding new logic to `audio/media_pipeline.rs`; new code should live under `audio/processor/{encoder.rs,streams.rs,frame_pipeline.rs}`.

## Research artifact: scripts/shrink.sh (do not integrate)
- Purpose: personal research harness for encoder settings exploration on macOS.
- Usage in this repo: reference only for ideas. Do not call or shell out from app.
- Mapping hints (script → app types):
  - `ENCODER=auto|fdk|apple` → `src/types/audio.ts::EncoderSettings.encoderType` (`'aac_at' | 'he_aac_v1' | 'he_aac_v2'`).
  - `FDK_VBR`, `BITRATE` → `EncoderSettings.bitrateKbps` and future VBR toggles (disabled today).
  - `CHANNELS` → `EncoderSettings.channels` (1 | 2).
  - `THREADS` → `EncoderSettings.threads`.

## Event contracts
- Event: `processing-progress`.
- Rust source of truth: `src-tauri/src/audio/progress/reporter.rs::ProgressEvent`.
- TS source of truth: `src/types/events.ts::ProcessingProgressEvent`.
- Backward-compat policy:
  - Additive fields only; optional in TS and defaulted in Rust.
  - Do not rename/remove existing fields without updating all listeners and UI surfaces.
- UI obligation: surface `eta_seconds` and `current_file` in `statusPanel`.
- Verification:
  - `RUST_LOG=debug npm run tauri dev` → process short sample → confirm stage transitions and UI renders.
  - Then `cargo test && cargo clippy -- -D warnings`.

## Frontend and Rust guardrails
- TypeScript:
  - Strict mode, explicit types, avoid `any`.
  - Expose encoder settings via `encoderPanel` provider with a typed `window.EncoderSettingsProvider` (see `src/types/audio.ts`).
- Rust:
  - Prefer `Result<T, AppError>` and `?`; `#![deny(clippy::unwrap_used)]`.
  - Keep internals non-`pub` unless required.
- Size/complexity (targets, not hard caps): file ≤ 400 LOC; function ≤ 55 LOC; ≤ 7 params; nesting ≤ 4.

## Backlog and reports
- Canonical backlog: `docs/planning_mapping/progress_bug_tracker.md`.
- Reports live under `docs/reports/`.
- Prioritization rubric:
  - P0: must fix/ship now; blocks current goal.
  - P1: valuable next; schedule soon.
  - P2+: polish or longer-term.
- Include minimal repro in reports: input files, settings, expected vs actual, logs, and event trace if applicable.

## Testing and coverage
- Source of truth: `docs/specs/test-coverage.md` (definition of “good coverage”).
- Where tests live: `src-tauri/tests/` for public APIs; inline tests allowed for private/internal code.
- Useful subsets: `cargo test path_validation`, `cargo test preview`, etc.
- Frontend checks: `tsc --noEmit`, `npm run build`.

## Build, run, and logs
- Frontend dev: `npm run dev`.
- App dev: `npm run tauri dev` (add `RUST_LOG=debug` for verbose backend).
- Production build (Tauri): `npm run app:build`.

## Collaboration preferences
- Validate non-trivial plans up front; prefer smallest safe diffs.
- Keep TS/Rust boundaries type-safe and explicit; update shared types if events change.
- No references to other AI guidance files; this AGENTS.md is the single source for agents.
- Community context: this follows the open `AGENTS.md` format standard (`https://agents.md/`).

---

Questions or changes? Propose edits in this file; do not modify the root `AGENTS.md` until approved.
