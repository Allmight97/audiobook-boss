# Fallback Register

Active register for intentional fallback behavior that is still enforced by repo checks.

| ID | Location | Trigger | Observe | Sunset | Issue |
| --- | --- | --- | --- | --- | --- |
| FB-001 | `src-tauri/src/metadata/reader.rs` | `mp4ameta` leaves key metadata fields unset | warning log with backfilled fields | 2026-03-31 | #196 |
| FB-002 | `src-tauri/src/metadata/cover_art/embedding.rs` | image bytes do not expose dimensions | warning log with format and byte length | 2026-03-31 | #197 |
| FB-003 | `src/ui/encoderPanel/logic.ts` | selected encoder becomes unavailable | console warning plus temporary UI override | 2026-04-30 | #198 |
| FB-004 | `src/ui/jobControls.ts` | `localStorage` is blocked or invalid | console warning on read or write failure | 2026-04-30 | #199 |
| FB-005 | `src-tauri/src/audio/processor/encoder/options/native.rs` | legacy `ABB_DISABLE_TWOOLOOP` env alias is still used | tests plus encoder startup logging | 2026-03-31 | #200 |
| FB-006 | `src-tauri/src/audio/processor/engine.rs` | legacy preview `early_stop` path is still toggled | info log when the branch is used | 2026-03-31 | #201 |
| FB-007 | `src-tauri/src/metadata/mp4ameta_bridge.rs`, `src-tauri/src/metadata/reader.rs` | movement-tag-only series metadata is encountered | metadata compatibility coverage | 2026-05-31 | #202 |
| FB-010 | `src-tauri/src/audio/buffer.rs` | encoder reports zero frame size | warning log when default `1024` frame size is applied | 2026-06-30 | #195 |
| FB-011 | `src-tauri/src/audio/settings_encoder.rs` | requested encoder is unavailable on the host | warning log with requested encoder and availability snapshot | 2026-06-30 | #195 |
| FB-012 | `src-tauri/src/audio/processor/finalize.rs` | atomic rename fails during output move | warning log for failed rename and copy-replace success | 2026-06-30 | #195 |
| FB-013 | `src/ui/encoderPanel/state.ts` | `localStorage` read or write is blocked or invalid | console warning on load or save fallback | 2026-06-30 | #195 |
| FB-016 | `scripts/perf/benches/metadata-lookup-latency.mjs` | real-network perf probe is disabled or fails | benchmark details include `fixture-fallback` | 2026-06-30 | #195 |
| FB-017 | `scripts/perf/shared/io.mjs` | optional perf artifacts are missing on disk | report output shows empty baseline or history bootstrap | 2026-06-30 | #195 |
| FB-018 | `scripts/checks.sh` | `.svelte` formatting still depends on Prettier | `bun run fmt:check` output and pre-commit signal | 2026-06-30 | #219 |
