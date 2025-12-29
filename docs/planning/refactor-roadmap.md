# Refactor Roadmap (Audio Pipeline Cleanup)

This roadmap sequences the three refactor issues so we can safely reduce LOC risk before adding new features.

## Sequence (Locked)

1) #123 — frame_pipeline.rs
2) #124 — context.rs
3) #125 — job_registry.rs

## Why This Order

- #123 is the largest module and sits on the critical audio path. It also unblocks #55 and #42.
- #124 is low risk and extracts PreviewConfig, which is shared by the preview pipeline.
- #125 touches concurrency core logic; best done after lower-risk wins and before queue work (#107, #71).

## Related Work

- After #123 merges: #55 (fast-path resampler investigation), #42 (preview hardening)
- After #125 merges: #107 (queue visibility), #71 (parallel batch)

## Verification Expectations

- Run full checks before merge when touching runtime behavior:
  - `cargo fmt --all -- --check` (src-tauri)
  - `cargo clippy -- -D warnings` (src-tauri)
  - `cargo test` (src-tauri)
  - `scripts/ensure-contract.sh`
  - `bun run build`

## Status

- #123: planned
- #124: planned
- #125: planned
