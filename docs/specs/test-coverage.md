# Test Coverage Standard (Good Coverage)

Focus on behavior and contracts over raw line percentages. The objective is to protect critical user flows and public contracts with fast, reliable tests.

## Scope and priorities

- P0 (must-have):
  - Path security validation (`audio::path_validation`) — accept only whitelisted audio, canonicalize, traverse-safe, symlink handling.
  - Progress contract (`processing-progress`) — event fields present; UI surfaces `current_file` and `eta_seconds`.
  - Metadata round-trip — read via Lofty; write preserves/embeds cover art; verify final M4B contains art and tags.
  - Encoder settings honored — TS→Rust boundary types enforced; bitrate/channels/profile applied.
  - Cancellation — command cancels cleanly; partial outputs cleaned up; UI transitions to cancelled.
  - Preview flow — preview seconds honored; preview file produced and optionally opened.

- P1 (should-have):
  - Output path probing/writability checks.
  - Error mapping — external errors mapped into `AppError` without leaking raw paths.
  - Progress smoothing/precision (frontend calculations don’t regress contract values).

- P2 (nice-to-have):
  - State machine transitions in UI (if introduced) are exhaustive and reject invalid states.

## Where tests live

- Primary: `src-tauri/tests/` (integration/public API oriented).
- Inline unit tests allowed for private/internal items otherwise unreachable.
- Keep file sizes small; co-locate helper fixtures per suite.

## Execution

- Full: from `src-tauri/`: `cargo test`.
- Useful subsets:
  - `cargo test path_validation`
  - `cargo test preview`
  - `cargo test settings_validation`
- Frontend type/build checks (repo root):
  - `tsc --noEmit`
  - `npm run build`

## Acceptance bars (what “good” means)

- Each P0 suite has at least:
  - A success-path test.
  - A failure-path test (invalid input, permission denied, etc.).
- Progress events:
  - Emit increasing `percentage` within [0,100].
  - Include `current_file` when processing multiple inputs.
  - Include `eta_seconds` when progress is between 0 and 100.
- Encoder settings:
  - A test that asserts bitrate/channels are reflected in the output stream parameters.
  - A test that invalid combinations are rejected with a typed error.
- Metadata:
  - Cover art present in final file; non-empty bytes; correct stream/tag location as per project convention.

## Data and fixtures

- Use bundled small media from `/media/` for speed and portability.
- Do not download external assets during tests.

## Maintenance

- When changing event payloads or encoder settings:
  - Update `src/types/*` and corresponding Rust types together.
  - Add/adjust tests in the affected P0 suite before merging.

## Quick pre-submit checklist (tests)

- `cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check`
- `tsc --noEmit && npm run build`
