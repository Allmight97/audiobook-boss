# v1 → v2 Migration Plan

Status: Draft (ready for implementation)
Branch baseline: `new_encoder`

## Goals
- Make v2 (`EncoderSettings`, `process_audiobook_files_v2`) the only boundary for processing.
- Stop mapping v2 → v1; route the pipeline via v2 directly.
- Remove v1 command/types from IPC boundary and docs.
- Keep progress events and UI contracts stable.

## Non‑Goals
- No behavior change to progress cadence, stage names, or UI event payloads.
- No feature expansion in this plan (encoder features/UI come after migration).

## Current State (Summary)
- v2 is used in the UI (`StatusPanel`) and validated in Rust, but mapped to v1 `AudioSettings` to run through the legacy pipeline.
- v1 commands still exist; v1 types appear at the IPC boundary in some places.
- UI has a parallel encoder type (`src/types/encoder.ts`) for future controls; boundary type resides in `src/types/audio.ts`.

## Risks and Impacts
- First‑order: IPC surface changes (removing v1) could break older callers (QA harness). Mitigated by sequential PRs and doc updates.
- Second‑order: Internal pipeline parameters (bitrate/channels/threads) sourced from v2 instead of v1.
- Third‑order: Reduced maintenance; single boundary eliminates contract drift.

## Architecture Principles
- Orthogonality: Separate output path ownership from encoder settings.
- KISS/DRY: One canonical boundary type for IPC; UI types adapt at invocation boundary.
- Fail fast: Preserve validation and path safety (`validate_input_audio_path`).

## Phased Execution

### PR A — Output config extraction (no behavior change)
Scope:
- Introduce an output config owned by processing context (derived from v2 payload: `outputDir` + deterministic filename).
- Replace reads of v1 `settings.output_path` with the new output config.
- Keep v2→v1 mapping for now.

Acceptance:
- All outputs continue to land at the same location.
- Run `scripts/quick-checks.sh` to verify fmt, clippy, contract, and TypeScript checks all pass.
- Tests pass; progress/events unchanged.

### PR B — Consume v2 in encoder setup
Scope:
- Replace usage of v1 `AudioSettings` with v2 `EncoderSettings` in encoder setup (bitrate, channels, threads, coder, afterburner, encoder selection).
- Remove `derive_v1_settings_from_v2` and the v2→v1 mapping path.

Acceptance:
- Feature parity for current flows (same output quality/format, stable progress emissions).
- v1 commands still compile but are unused.
- Run `scripts/quick-checks.sh` to verify fmt, clippy, contract, and TypeScript checks all pass.

### PR C — Remove v1 commands and boundary types
Scope:
- Remove `process_audiobook_files`, `validate_audio_settings` and any v1 IPC payloads.
- Update `window.testCommands`, docs, and ensure no v1 references.
- Clean up unused imports and code artifacts left from PR A/B refactoring (e.g., unused `SampleRateConfig` imports in command handlers that no longer construct v1 settings explicitly).

Acceptance:
- Repo builds with only v2.
- Run `scripts/quick-checks.sh` to verify fmt, clippy, contract, and TypeScript checks all pass (catches unused imports, formatting issues, and contract mismatches).
- Ensure-contract validation passes.

### PR D — Unify front‑end encoder types
Scope:
- Keep `src/types/audio.ts::EncoderSettings` as the boundary type.
- Treat `src/types/encoder.ts` as UI‑only; provide an adapter `toBoundaryEncoderSettings(ui: EncoderSettingsV2): EncoderSettings` if necessary.
- Ensure all `invoke()` calls pass the boundary `EncoderSettings` shape.

Acceptance:
- No direct serialization of UI‑only type across IPC.
- UI provider (if present) returns boundary type.
- Run `scripts/quick-checks.sh` to verify fmt, clippy, contract, and TypeScript checks all pass.

### PR E — Cleanup and docs (v2‑only)
Scope:
- Remove v1 mentions from docs (`README.md`, `AGENTS.md`, `docs/external-apis/*`).
- Confirm event contract docs match runtime (already aligned in prior PR).

Acceptance:
- Docs match code.
- Run `scripts/quick-checks.sh` to verify fmt, clippy, contract, and TypeScript checks all pass.

## Contracts and Compatibility
- Events: `processing-progress` remains unchanged. Stage ranges are 0–10 / 10–79 / 90–95 / 95–100.
- Path validation: continue using `audio::path_validation::validate_input_audio_path()`.
- UI filtering vs backend allowlist (MP3/M4A/M4B/AAC vs WAV/FLAC): unchanged.

## Testing & Validation
- Quick checks: `scripts/quick-checks.sh` (fmt, clippy, contract, tsc).
- Rust: from `src-tauri/` run `cargo test`, `cargo clippy -- -D warnings`.
- Integration sanity: process short samples; verify event cadence and stage transitions.

## Rollback Plan
- Each PR is revertible independently. No db/schema migrations.

## Timeline (suggested)
- PR A: 1–2 days
- PR B: 2–3 days (most of the integration swap)
- PR C: 1 day
- PR D: 1 day
- PR E: 0.5 day

