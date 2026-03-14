---
name: fix branch regressions
overview: "Address the three highest-priority regressions on the branch: eager metadata reads that can abort processing, inconsistent unavailable-encoder handling across UI/backend, and incomplete `bundled-ffmpeg` enforcement in the macOS build wrapper."
todos:
  - id: processing-preflight
    content: Refactor processing preflight so metadata read failures do not abort batch/merge naming before job execution.
    status: completed
  - id: encoder-consistency
    content: Unify unavailable-encoder handling across UI state, validation, and processing execution.
    status: completed
  - id: build-wrapper-features
    content: Make macOS build wrapper merge existing `--features` with `bundled-ffmpeg` and add regression tests.
    status: completed
  - id: verify-regressions
    content: Run targeted tests and repo-standard checks to confirm the three regressions are closed.
    status: completed
isProject: false
---

# Fix P0-P2 Branch Regressions

## Scope

Restore resilient processing behavior, make encoder selection/validation consistent across frontend and backend, and harden the macOS build wrapper so self-contained builds stay self-contained.

## 1. Fix P0: Stop eager metadata reads from aborting processing

- Update [src-tauri/src/commands/audio_processing.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/commands/audio_processing.rs) so output naming no longer requires `crate::metadata::read_metadata()` before jobs are even scheduled.
- Keep filename generation resilient by relying on the existing source-path fallback in [src-tauri/src/audio/output_path.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/output_path.rs) when metadata cannot be read or is not explicitly supplied.
- Refactor `resolve_processing_metadata()` / `resolve_processing_metadata_for_input()` so source metadata is loaded only when the processing path truly needs merged metadata, and failures are either:
  - deferred to the per-job execution path, or
  - handled as a metadata fallback without blocking naming for otherwise valid inputs.
- Preserve the batch partial-failure model in [src-tauri/src/commands/audio_processing.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/commands/audio_processing.rs): one problematic file should not pre-abort the entire batch during preflight.
- Add regression coverage in [src-tauri/tests/integration_processing_flow_tests.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/tests/integration_processing_flow_tests.rs) or a nearby targeted test file for:
  - batch processing with a file whose metadata read fails but whose path is otherwise valid for naming fallback,
  - merge/batch behavior when only metadata overlay is provided,
  - output naming still using source filename / `Unknown Author` fallback when metadata is unavailable.

## 2. Fix P1: Make unavailable manual encoder selections deterministic

- Align encoder behavior across:
  - [src-tauri/src/audio/settings_encoder.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/settings_encoder.rs)
  - [src-tauri/src/commands/audio.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/commands/audio.rs)
  - [src-tauri/src/commands/audio_processing.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/src/commands/audio_processing.rs)
  - [src/ui/encoderPanel/logic.ts](/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/logic.ts)
  - [src/ui/encoderPanel/state.svelte.ts](/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/state.svelte.ts)
- Decide and implement one consistent rule for explicit unavailable encoders:
  - preferred: fail early with a clear validation error for any unavailable explicit encoder (`fdk_he_aac`, `aac_at`, `native_aac`), while `auto` continues to resolve dynamically.
- Add frontend normalization so stale persisted `flavor` values do not silently remain selected when the current availability says that option is disabled.
- Ensure `validate_encoder_settings` and `process_audiobook_files` share the same availability rule, so the user cannot pass UI validation and then fail later during processing with a different outcome.
- Extend tests in:
  - [src-tauri/tests/unit_settings_encoder_tests.rs](/Users/jstar/Projects/audiobook-boss/src-tauri/tests/unit_settings_encoder_tests.rs)
  - [src/lib/tauri-client.test.ts](/Users/jstar/Projects/audiobook-boss/src/lib/tauri-client.test.ts) if boundary behavior changes
  - [src/ui/**tests**/encoderPanel-behavior.test.ts](/Users/jstar/Projects/audiobook-boss/src/ui/__tests__/encoderPanel-behavior.test.ts)
  - [src/ui/**tests**/encoderPanel-native-warning.test.ts](/Users/jstar/Projects/audiobook-boss/src/ui/__tests__/encoderPanel-native-warning.test.ts)
- Cover at least:
  - persisted manual `aac_at` on a host where `aac_at` is unavailable,
  - persisted manual `native_aac` on a build where native AAC is unavailable,
  - validation and processing both returning the same explicit error text.

## 3. Fix P2: Guarantee `bundled-ffmpeg` is actually present in macOS builds

- Replace the current `ensureFlag()` behavior in [scripts/build-app.ts](/Users/jstar/Projects/audiobook-boss/scripts/build-app.ts) with feature-aware merging logic.
- If the caller already passes `--features`, append `bundled-ffmpeg` to the existing comma-separated feature set instead of leaving the existing list untouched.
- Preserve caller intent for unrelated flags and feature combinations.
- Extend [scripts/build-app.test.ts](/Users/jstar/Projects/audiobook-boss/scripts/build-app.test.ts) with cases for:
  - no `--features` present,
  - existing `--features custom-protocol`,
  - existing list already containing `bundled-ffmpeg`,
  - repeated / malformed edge cases handled predictably.

## Validation

- Run targeted tests first:
  - `cargo test --manifest-path src-tauri/Cargo.toml integration_processing_flow_tests unit_settings_encoder_tests -- --nocapture`
  - `bun run test -- src/ui/__tests__/encoderPanel-behavior.test.ts src/ui/__tests__/encoderPanel-native-warning.test.ts src/lib/tauri-client.test.ts`
  - `bun test scripts/build-app.test.ts`
- Then run repo-required verification for code changes:
  - `scripts/checks.sh standard`
  - `bun run bindings:check` if command/error contract shapes change.

## Notes

- Keep the fix minimal and contract-safe: no new fallback policy unless the behavior is explicit, observable, and tested.
- Prefer sharing availability checks in one backend helper so UI validation and execution cannot drift again.
