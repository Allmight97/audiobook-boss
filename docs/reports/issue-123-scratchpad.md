# Issue #123 Scratchpad

## Scope (as written)
- Split `src-tauri/src/audio/processor/frame_pipeline.rs` into smaller modules.
- Move preview/chapter state into a new module (likely `preview_state.rs`).
- Move inline tests into `src-tauri/tests/unit/`.
- Preserve behavior; no feature changes.

## Related Issues (to investigate after PR)
- #55: fast-path resampler bypass behavior (lives in frame pipeline)
- #42: preview hardening (preview/chapter logic)
- #124: context split (Prep/research only in this session)

## Baseline Notes
- Current `frame_pipeline.rs` contains PreviewState, ChapterMarker, sanitize_chapter_title, PreviewAction, pipeline functions, and inline tests.
- Public re-exports in `src-tauri/src/audio/processor/mod.rs` include preview types.
- Preview logic is used in `src-tauri/src/audio/processor/engine.rs`.

## Risks
- Import path changes for preview types could cascade into `engine.rs` and re-exports.
- Tests must be moved to external test modules (Clean Source rule).

## Plan Checkpoints
- Keep behavior identical; avoid touching encode/resample logic.
- Maintain public re-exports for preview types.
- Move tests to `src-tauri/tests/unit/processor/frame_pipeline_tests.rs`.

## Notes for #124 Prep
- `context.rs` split will move `PreviewConfig` to `audio/preview_config.rs`.
- After #124, `preview_state.rs` will need import updates.


## Checks
- scripts/quick-checks.sh: pass.
- scripts/coverage.sh: pass (tarpaulin + vitest).
  - Rust report: coverage/rust/tarpaulin-report.html
  - TS report: coverage/typescript/index.html

## Issue #55 (fast-path resampler bypass) - Current Status
- Fast-path logic still exists in `frame_pipeline.rs` and is gated by `ABB_DISABLE_FASTPATH`.
- No repo-level default sets `ABB_DISABLE_FASTPATH=1`; disabling is external to repo.
- There is an integration test `src-tauri/tests/integration/pipeline_fastpath.rs`, but it only exercises encode flow and does not validate audio quality parity.
- Acceptance criteria still unmet (needs A/B runs and audio comparison). Recommend keeping #55 open.

## Issue #42 (preview hardening) - Current Status
- Multi-file preview integration test: not present; only `preview_30s_integration.rs` exists.
- `PreviewAction` is still missing `#[must_use]`.
- Preview duration has no upper-bound validation in `src-tauri/src/commands/audio.rs`.
- `sanitize_chapter_title()` still does not include ':' or '\t' in the replace set.
- Overall: all checklist items still open; scope unchanged.
