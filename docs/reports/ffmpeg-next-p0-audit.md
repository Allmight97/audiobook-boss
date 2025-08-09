### ffmpeg-next P0 audit (branches vs base)

Scope: Verify P0 criteria for `safe-ffmpeg` feature with `ffmpeg-next` implementation, tests, and docs. Base branch: `pre_ffmpegnext` (current). Sub-branches: `ffmpeg-next-p0-implementation` @ a3e8683, `gpt5-p0_ffmpegnext` @ 2f995c9.

#### TL;DR
- Which code is better right now: gpt5-p0_ffmpegnext. It actually implements a minimal `FfmpegNextProcessor` that merges inputs into AAC in an M4B container, emits progress, and honors cancel behind compile-time gating.
- ffmpeg-next-p0-implementation is a scaffold; it initializes ffmpeg-next then returns an unimplemented error. It appears to set up for future work but does not complete P0 tasks.
- Both branches break the default build by introducing a bundled-FFmpeg resource requirement in dev that isn’t present locally. This violates “default build unchanged”. Fix packaging before merging.

---

### P0 criteria summary

| Criterion                                                                              | ffmpeg-next-p0-implementation                                                                                                                                                                                                                                               | gpt5-p0_ffmpegnext                                                                                                       |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Cargo feature `safe-ffmpeg` exists; `ffmpeg-next` is optional; default build unchanged | FAIL: feature present, but default build fails (missing bundled FFmpeg resource). Evidence: `src-tauri/Cargo.toml` adds `ffmpeg-next = { optional = true }` and `safe-ffmpeg = ["dep:ffmpeg-next"]`; build error: missing `binaries/ffmpeg-universal-aarch64-apple-darwin`. | FAIL: same feature gating present; same default build failure (missing bundled FFmpeg resource).                         |
| Minimal `FfmpegNextProcessor`: merge inputs → AAC/M4B; emits progress; honors cancel   | FAIL: scaffold only; returns "not yet implemented" error after `ffmpeg_next::init()`.                                                                                                                                                                                       | PASS: full decode→resample→AAC encode→mux implemented; emits progress every ~200ms; checks cancel between stages.        |
| Selection via compile-time feature only; no runtime change by default                  | PASS: `#[cfg(feature = "safe-ffmpeg")]` processor selection; default uses shell.                                                                                                                                                                                            | PASS: same compile-time gating; default build path unchanged by code.                                                    |
| Feature-gated test using repo fixtures; no network                                     | PARTIAL: only structure tests; lacks happy-path merge assertion. Uses `../media/01 - Introduction.mp3` in integration tests.                                                                                                                                                | PASS: `src-tauri/tests/unit/audio/ffmpegnext_tests.rs` feature-gated; uses `../media/01 - Introduction.mp3`; no network. |
| Docs updated (migration, hand-off: build matrix, usage, limits, next steps)            | PASS: updated and claim P0 completion (claim does not match code).                                                                                                                                                                                                          | PASS: updated; claims P0 complete behind feature (matches code).                                                         |

#### Key evidence

- Feature gating and optional dependency (both branches)
  - `src-tauri/Cargo.toml`: `ffmpeg-next = { version = "7", optional = true }`; `[features] safe-ffmpeg = ["dep:ffmpeg-next"]`.

- Compile-time processor selection (both branches)
  - `src-tauri/src/audio/processor.rs`: `#[cfg(feature = "safe-ffmpeg")] let processor = FfmpegNextProcessor; #[cfg(not(feature = "safe-ffmpeg"))] let processor = ShellFFmpegProcessor;`.

- ffmpeg-next-p0-implementation: scaffold only
  - `src-tauri/src/audio/media_pipeline.rs` (feature block): calls `ffmpeg_next::init()` then returns `AppError::General("FfmpegNextProcessor: Core decode/encode pipeline not yet implemented...")`.

- gpt5-p0_ffmpegnext: functional minimal encode path
  - `src-tauri/src/audio/media_pipeline.rs` (feature block):
    - Initialize once: `ff::init()`.
    - Derive target sample rate/channels (Auto → from first input).
    - Create output context + AAC encoder; set bit rate, rate, channels, sample format, time base; write header.
    - For each input: open input → best audio stream → decoder → resampler to encoder fmt → send frames to encoder → receive packets → set stream/index/timebase → write interleaved.
    - Emit progress via `ProgressEmitter` every ~200ms using accumulated PTS vs `plan.total_duration`.
    - Check `context.is_cancelled()` in outer loops; flush decoder and encoder; write trailer.

- Tests with repo media
  - gpt5: `src-tauri/tests/unit/audio/ffmpegnext_tests.rs` is `#![cfg(feature = "safe-ffmpeg")]`; happy-path test asserts output file exists for `../media/01 - Introduction.mp3` if present; separate error-path test for missing input.
  - ffmpeg-next-p0-implementation: integration tests under feature mainly validate structure/selection; they don’t perform an actual merge.

---

### Build/test results

Commands run per branch (from `src-tauri/`):

```
cargo fmt --check
cargo clippy -- -D warnings
cargo build
cargo test
cargo clippy --features safe-ffmpeg -- -D warnings
cargo build  --features safe-ffmpeg
cargo test   --features safe-ffmpeg
```

- Base branch (pre_ffmpegnext):
  - Build: OK; Test: OK (41 passed). Only dead_code warnings in non-new code.

- ffmpeg-next-p0-implementation (a3e8683):
  - Build: FAIL. Error excerpt:
    - `resource path 'binaries/ffmpeg-universal-aarch64-apple-darwin' doesn't exist` (from project build script via Tauri tooling).
  - Tests: not run (blocked by build).

- gpt5-p0_ffmpegnext (2f995c9):
  - Build: FAIL. Same error: missing `binaries/ffmpeg-universal-aarch64-apple-darwin`.
  - Tests: not run (blocked by build).

Observations: The default build breaks in both sub-branches due to a packaging/resource change (bundled FFmpeg) that isn’t present locally. The base branch does not exhibit this failure on the same machine.

---

### Gaps and suggested fixes

- Default build unchanged (P0): FAIL in both branches
  - Gap: Build requires a bundled FFmpeg resource in dev (`binaries/ffmpeg-universal-aarch64-apple-darwin`), which isn’t present.
  - P0 fix (choose one, do not change defaults):
    - Dev-gate the external binary requirement so plain `cargo build/test` works without bundled FFmpeg. Keep bundling only for packaging.
    - Or provide a local symlink setup step (documented) to meet the expected path on Apple Silicon (e.g., symlink `$(which ffmpeg)` to the expected filename under `src-tauri/binaries/`).

- Minimal ffmpeg-next path (P0):
  - ffmpeg-next-p0-implementation: Not implemented; returns a placeholder error. Add the minimal decode→encode pipeline (can cherry-pick from gpt5).

- Feature-gated tests (P0):
  - ffmpeg-next-p0-implementation: Add a happy-path test that merges the repo fixture and asserts output exists, behind `#[cfg(feature = "safe-ffmpeg")]`.

- Docs accuracy:
  - ffmpeg-next-p0-implementation docs claim P0 completed; code does not meet P0 minimal encode requirement. Adjust wording or complete implementation to match.

- Lint/format:
  - `cargo fmt --check` shows diffs in tests in both branches; ensure formatting clean. Run clippy on both feature sets and resolve warnings.

---

### Risk and rollback

- Rollback: Build without `--features safe-ffmpeg` (default) to use shell engine (once default build is restored). No runtime behavior changes by default.
- Disable: Don’t enable feature for release until happy-path test is green on macOS ARM.
- Revert: If needed, revert sub-branch commits to restore base behavior quickly.

---

### Recommendation

- Merge choice: Prefer `gpt5-p0_ffmpegnext` after fixing the default build packaging issue and ensuring fmt/clippy pass.
  - Rationale: Implements the P0 minimal ffmpeg-next pipeline with progress and cancel plus proper feature-gated tests using repo media. The other branch is a scaffold.

- Alternative: Cherry-pick plan onto a fresh branch from base (recommended commit order)
  1) Features: optional `ffmpeg-next` + `safe-ffmpeg` feature (either branch).
  2) Skeleton: `MediaProcessor` + compile-time selection (present in both).
  3) Minimal encode: copy `FfmpegNextProcessor` implementation from `gpt5-p0_ffmpegnext` (`src-tauri/src/audio/media_pipeline.rs`, feature block).
  4) Tests: copy `src-tauri/tests/unit/audio/ffmpegnext_tests.rs` (feature-gated happy and error paths using repo media).
  5) Docs: keep migration + hand-off edits; update to reflect the real build matrix and constraints.
  6) Packaging: ensure default `cargo build/test` doesn’t require bundled FFmpeg; bundle only for packaging.

- Exact commit(s) to merge or cherry-pick:
  - `gpt5-p0_ffmpegnext`: 2f995c9 "Implement ffmpeg-next processor with feature gating and add corresponding tests." (plus packaging fix commit before merge)
  - Avoid merging a3e8683 without adding the minimal encode path.

---

### PR comment-ready summary

- P0 audit:
  - `gpt5-p0_ffmpegnext`: Implements minimal ffmpeg-next pipeline behind `safe-ffmpeg`, with progress + cancel and feature-gated tests using repo media. Default build currently broken by missing bundled-FFmpeg resource.
  - `ffmpeg-next-p0-implementation`: Scaffold only; minimal encode not implemented; also breaks default build. Docs claim P0 complete but code doesn’t meet P0 minimal encode/test criteria.
- Required before merge:
  - Restore default build unchanged (remove/dev-gate bundled FFmpeg requirement in dev or provide a local symlink step).
  - Run fmt/clippy clean.
- Recommendation:
  - Merge `gpt5-p0_ffmpegnext` after packaging fix; or cherry-pick 2f995c9 (processor + tests) to a new branch, apply packaging fix, then merge.


