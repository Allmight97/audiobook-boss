# Fallback Register

Active register for fallback behavior that is still enforced by repo checks.
Keep entries here only when they materially protect product behavior, output integrity, real external-file interoperability, or the main release-quality gate.
Best-effort UI preference persistence and optional perf/bootstrap tolerance should be trimmed out instead of living here indefinitely.
Repo checks validate register sunsets, source-adjacent marker sunsets, and any renewal dates as real calendar dates.

| ID | Location | Trigger | Observe | Sunset | Issue | Audit Status |
| --- | --- | --- | --- | --- | --- | --- |
| FB-001 | `src-tauri/src/metadata/reader.rs` | `mp4ameta` read fails | warning log with primary read failure reason before ffmpeg fallback | 2026-05-31 | #196 | REVIEW — useful mislabeled-file tolerance today, but replaceable by better container classification |
| FB-007 | `src-tauri/src/metadata/mp4ameta_bridge.rs`, `src-tauri/src/metadata/reader.rs` | movement-tag-only series metadata is encountered | metadata compatibility coverage | 2026-05-31 | #202 | RETAIN — genuine external-file interoperability requirement |
| FB-010 | `src-tauri/src/audio/buffer.rs` | encoder reports zero frame size | warning log when default `1024` frame size is applied | 2026-06-30 | #195 | RETAIN — encoder boundary still needs explicit `frame_size=0` handling |
| FB-012 | `src-tauri/src/audio/processor/finalize.rs` | atomic rename fails during output move | warning log for failed rename and copy-replace success | 2026-06-30 | #195 | RETAIN — output integrity across cross-volume rename failures |
| FB-017 | `scripts/perf/shared/io.mjs` | optional perf artifacts are missing on disk | report output shows empty baseline or history bootstrap | 2026-06-30 | #195 | REVIEW — optional perf bootstrap behavior; should leave active register |
| FB-018 | `scripts/checks.sh` | `.svelte` formatting still depends on Prettier | `bun run fmt:check` output and pre-commit signal | 2026-06-30 | #219 | RETAIN FOR NOW — main Svelte format gate still depends on Prettier |

Renewals, when needed, stay compact: append `renewal=YYYY-MM-DD; reason=...` to the Audit Status cell and make sure the renewal date is a valid calendar date that extends the sunset.
