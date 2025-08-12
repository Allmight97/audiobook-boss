## Pre-Planning Audit: Feasibility of Combining P1.1.4, P1.1.5, and P1.1.6

### Overview
This audit evaluates merging the following tasks into a single, coherent plan:
- P1.1.4: Split `audio/cleanup.rs` into `cleanup/{guard.rs, ops.rs, mod.rs}`
- P1.1.5: Split `commands/mod.rs` into `commands/{audio.rs, metadata.rs, system.rs}`
- P1.1.6: Split `ui/fileList.ts` and `ui/statusPanel.ts` into sub-files with aggregator facades

Conclusion: Merging is feasible with low-to-moderate risk if we preserve public surfaces via re-exports (Rust) and aggregator shims (TS) and validate with staged commits and builds.

### Scope and Key Couplings
- Rust
  - `audio/cleanup.rs`: Used via `crate::audio::cleanup::CleanupGuard` and re-exported in `audio/mod.rs`. `ProcessGuard` is feature-gated to `any(test, feature = "safe-ffmpeg")`.
  - `commands/mod.rs`: Contains mixed domains; commands are registered by name in `src-tauri/src/lib.rs` using `tauri::generate_handler!`.
  - `media_pipeline.rs`: References `CleanupGuard` for partial-output deletion on failure/cancel.
- TypeScript
  - `ui/fileList.ts`: Exports `displayFileList`, `toggleFileSort`, `clearAllFiles`, plus shared state `currentFileList`, `selectedFileIndex`. Used by `main.ts` and `outputPanel.ts`.
  - `ui/statusPanel.ts`: Class-based component; imports `currentFileList` and `getCurrentAudioSettings`; listens for `'processing-progress'`.

### Findings
- P1.1.4: `cleanup.rs` (~500 LOC) is self-contained. Splitting into `guard.rs` and `ops.rs` with `pub use` preserves API (`crate::audio::cleanup::CleanupGuard`). Keep feature gates for `ProcessGuard`.
- P1.1.5: `commands/mod.rs` (~280 LOC) can be split by domain. Keep function names and `#[tauri::command]` attributes, and re-export them from `commands/mod.rs` to avoid touching `lib.rs`.
- P1.1.6: `fileList.ts` (~540 LOC) and `statusPanel.ts` (~440 LOC) can be split into folders and re-aggregated by `ui/fileList.ts` and `ui/statusPanel.ts` to preserve imports. Avoid duplicating state; centralize in `state.ts`.

### Pros of a Single Merged Plan
- Unified API-stability strategy (re-exports/aggregators) reduces contract churn across Rust and TS.
- Shared refactor pattern minimizes context switching; fewer PRs to review.
- One consolidated validation cycle (cargo test/clippy default + `--features safe-ffmpeg`, TS build).

### Cons and Risks
- Larger blast radius across Rust and TS in one change.
- Longer build/validate loop per iteration.
- UI coupling risk: `currentFileList` and named exports must be preserved; command names must remain identical for `invoke()` calls.

### Risk Mitigations
- Preserve public surfaces: re-export Rust items; keep `#[tauri::command]` names; TS aggregator shims re-export same symbols.
- Stage commits: split by component (cleanup → commands → TS `fileList` → TS `statusPanel`), run tests/builds after each.
- Snapshot sizes using `scripts/sg/size_budget.sh` before and after each stage to confirm line-count goals and detect regressions.
- Contract checks: Ensure `lib.rs` `generate_handler!` command list is unchanged and all TS `invoke(...)` names still resolve.

### Recommendation
Proceed with a merged plan using the mitigations above. A conservative alternative is pairing P1.1.4 + P1.1.5 together and P1.1.6 afterward; however, the full merge remains reasonable given re-export/aggregator options.

### Validation Summary
- Rust: `cargo test`, `cargo clippy -- -D warnings` (default) and `cargo test --features safe-ffmpeg`, `cargo clippy --features safe-ffmpeg -- -D warnings`.
- TS: `npm run build`, manual UI smoke via `window.testCommands` in `src/main.ts`.
- No event or command contract changes required.

> Note: This report documents feasibility only. No code changes were made.


