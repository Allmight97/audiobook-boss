# Fallback Register

Active register for intentional fallback behavior that is still enforced by repo checks.

| ID | Location | Trigger | Observe | Sunset | Issue | Audit Status |
| --- | --- | --- | --- | --- | --- | --- |
| FB-001 | `src-tauri/src/metadata/reader.rs` | `mp4ameta` read fails | warning log with primary read failure reason before ffmpeg fallback | 2026-05-31 | #196 | OK — primary-series hydration removed; ffmpeg is now only the hard read-failure fallback for MP4/M4B |
| FB-004 | `src/ui/jobControls.ts` | `localStorage` is blocked or invalid | console warning on read or write failure | 2026-04-30 | #199 | OK |
| FB-007 | `src-tauri/src/metadata/mp4ameta_bridge.rs`, `src-tauri/src/metadata/reader.rs` | movement-tag-only series metadata is encountered | metadata compatibility coverage | 2026-05-31 | #202 | OK — genuine external-file interop requirement |
| FB-010 | `src-tauri/src/audio/buffer.rs` | encoder reports zero frame size | warning log when default `1024` frame size is applied | 2026-06-30 | #195 | OK — genuine ffmpeg API edge case |
| FB-012 | `src-tauri/src/audio/processor/finalize.rs` | atomic rename fails during output move | warning log for failed rename and copy-replace success | 2026-06-30 | #195 | OK — cross-volume rename legitimately fails on macOS |
| FB-013 | `src/ui/encoderPanel/state.ts` | `localStorage` read or write is blocked or invalid | console warning on load or save fallback | 2026-06-30 | #195 | OK |
| FB-016 | `scripts/perf/benches/metadata-lookup-latency.mjs` | real-network perf probe is disabled or fails | benchmark details include `fixture-fallback` | 2026-06-30 | #195 | OK |
| FB-017 | `scripts/perf/shared/io.mjs` | optional perf artifacts are missing on disk | report output shows empty baseline or history bootstrap | 2026-06-30 | #195 | OK |
| FB-018 | `scripts/checks.sh` | `.svelte` formatting still depends on Prettier | `bun run fmt:check` output and pre-commit signal | 2026-06-30 | #219 | OK |

Renewals, when needed, stay compact: append `renewal=YYYY-MM-DD; reason=...` to the Audit Status cell and make sure the renewal date is a valid calendar date that extends the sunset.
