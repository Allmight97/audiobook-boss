# Encoding & Processing — Test Gaps and High-Value Additions

Last updated: 2025-08-22

Goal: Identify missing tests and propose high-value additions to cover the new encoder options and existing processing pipeline.

## Current signals
- Unit tests around metadata dictionary conversion and cover art helpers
- Integration tests for cover art embedding and path validation
- Minimal/no tests for encoder option behavior, profiles, threads, and validation

## High-value tests to add

### 1) Validation and contract tests (unit)
- HE-AAC v2 stereo enforcement
  - Given `encoder_type=HeAacV2` and `channels=1`, expect validation error
- Bitrate whitelist (56, 64, 72, 80, 88, 96) for V2 payload
- Thread setting guard
  - `Fixed(0)` or negative → error
 - V2 command payload schema
   - Round-trip serde for `EncoderSettings` (snake/camel where applicable); ensure legacy v1 is unchanged

### 2) Encoder mapping tests (unit)
- Profile mapping best-effort
  - Setting HE-AAC v1 → `av_opt_set_int(profile=HE)` returns 0 (if supported), otherwise logs a warning
  - Setting HE-AAC v2 → `av_opt_set_int(profile=HE_V2)` returns 0 (if supported)
- AAC coder mapping
  - `aac_coder=twoloop` and `fast` via `av_opt_set`, error code handling → logs debug or warn
- Threads mapping
  - Auto → threads=0; Off → 1; Fixed(n) → n; verify via `av_opt_get_int` if feasible, else capture logs
 - Afterburner feature flag
   - When FDK is not active, setting `afterburner=true` yields INFO "ignored"; when feature flag is on and encoder is FDK, verify option is applied (if CI image supports it, else mark as conditional)

### 3) Encoder selection tests (integration)
- macOS AAC-AT selection
  - When selecting AAC-AT on macOS, ensure encoder-by-name is resolved and open succeeds; if not available, log an INFO fallback
  - Verify output contains expected encoder tag (if available) or rely on log assertion
- Native AAC profiles
  - HEv1/HEv2 open without error and produce playable M4B at target bitrate
 - HE-AAC v2 stereo lock UX
   - UI disables channel control and displays explanatory toast; backend rejects mono if bypassed

### 4) End-to-end smoke tests (integration)
- V2 command path with small input clip (10-20s)
  - HE-AAC v1 mono 64k twoloop
  - HE-AAC v2 stereo 64k fast
  - AAC-AT mono 64k with threads auto
- Assertions
  - Process completes; file exists; container metadata present
  - Log contains encoder summary with resolved params

### 5) UI contract tests (lightweight)
- TypeScript types compile and encode payload matches Rust serde expectations (`camelCase` vs `snake_case` mapping decisions)
- UI disables controls when AAC-AT selected; HEv2 forces stereo

## Infrastructure helpers
- Log capture utility for Rust integration tests to assert on INFO/DEBUG lines
- Tiny fixture audio files (2–3s) for fast tests
- Conditional tests with `#[cfg(target_os = "macos")]` for AAC-AT

## Open items / questions
- Will we support `libfdk_aac`? Decision: future optional feature flag; tests should treat it as ignored unless FDK encoder is active.
- Do we want a v1 command to co-exist long-term? If not, plan deprecation tests and migration guidance.

