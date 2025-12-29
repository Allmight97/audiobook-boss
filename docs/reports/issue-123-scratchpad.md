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
