# Fallback Register

This register tracks every intentional fallback/shim under the strict policy in `AGENTS.md`.

Required fields per fallback:
- `trigger`: concrete condition that activates fallback behavior
- `observe`: signal proving fallback usage is visible (log/metric/test)
- `sunset`: explicit removal/revalidation bound
- `issue`: GitHub tracking issue

| ID | Location | Trigger | Observe | Sunset | Issue | Status |
|---|---|---|---|---|---|---|
| FB-001 | `src-tauri/src/metadata/reader.rs` | mp4ameta succeeds but series/cover fields are missing | warn log with backfilled field list | 2026-03-31 | #196 | Active |
| FB-002 | `src-tauri/src/metadata/cover_art/embedding.rs` | cover-art dimensions cannot be parsed from image bytes | warn log with format + byte length | 2026-03-31 | #197 | Active |
| FB-003 | `src/ui/encoderPanel/logic.ts` | selected encoder becomes unavailable and must auto-switch | console warn + temporary UI hint override | 2026-04-30 | #198 | Active |
| FB-004 | `src/ui/jobControls.ts` | localStorage unavailable/blocked, or invalid persisted value | console warn markers for read/write/parse paths | 2026-04-30 (revalidate) | #199 | Active |
| FB-005 | `src-tauri/src/audio/processor/encoder/options/native.rs` | legacy env alias `ABB_DISABLE_TWOOLOOP` still present in local setups | test coverage + encoder startup logs | 2026-03-31 | #200 | Active |
| FB-006 | `src-tauri/src/audio/processor/engine.rs` | legacy `early_stop` path still toggled in preview loop | info log when branch is engaged | 2026-03-31 | #201 | Active |
| FB-007 | `src-tauri/src/metadata/mp4ameta_bridge.rs`, `src-tauri/src/metadata/reader.rs` | legacy movement tag fields are present without canonical series tags | metadata compatibility tests + register tracking | 2026-05-31 (revalidate) | #202 | Active |
| FB-008 | `src/lib/bridge.ts`, `src-tauri/src/commands/audio.rs` | preserve stable snake_case command surface during architecture transition | behavior-contract compatibility guards + generated binding drift checks | framework migration phase 2 completion, no later than 2026-05-31 | #203 | Active |
| FB-009 | `scripts/bench-statuspanel-render-lookup.mjs` | legacy local command still targets old perf entrypoint | grep/workflow checks for shim references | 2026-03-31 | #204 | Active |
| FB-010 | `src-tauri/src/audio/buffer.rs` | encoder reports zero frame size and accumulator must continue safely | warn log when default frame size `1024` is applied | 2026-06-30 (revalidate) | #195 | Active |
| FB-011 | `src-tauri/src/audio/settings_encoder.rs` | requested encoder unavailable on current host/runtime | warn log with requested encoder + availability snapshot | 2026-06-30 (revalidate) | #195 | Active |
| FB-012 | `src-tauri/src/audio/processor/finalize.rs` | atomic rename fails during final output move | warn rename failure + info copy-replace success logs | 2026-06-30 (revalidate) | #195 | Active |
| FB-013 | `src/ui/encoderPanel/state.ts` | localStorage read/write blocked or invalid | console warn markers for load/save fallback paths | 2026-06-30 (revalidate) | #195 | Active |
| FB-014 | `src/ui/statusPanel/processing.ts` | encoder settings provider unavailable in runtime context | console warn when sanitized defaults are injected | 2026-06-30 (revalidate) | #195 | Active |
| FB-015 | `src/lib/bridge.ts` | non-Tauri runtime requires DEV mocks / non-DEV no-op rejection path | bridge init + non-Tauri warning logs | 2026-06-30 (revalidate) | #195 | Active |
| FB-016 | `scripts/perf/benches/metadata-lookup-latency.mjs` | real-network perf probe disabled/fails and synthetic fallback is used | benchmark result details include `fixture-fallback` + reason | 2026-06-30 (revalidate) | #195 | Active |
| FB-017 | `scripts/perf/shared/io.mjs` | optional perf artifact files missing (`ENOENT`) | report output demonstrates empty baseline/history bootstrap path | 2026-06-30 (revalidate) | #195 | Active |

## Migration Hooks (Svelte + Tailwind Track)

- FB-008 is migration-bound and must be removed when framework migration reaches typed component/service paths.
- FB-014 and FB-015 should be re-evaluated during framework migration once component-level state management replaces global provider/runtime branching.
- Framework migration PR checklist must include:
  - no new dual-key fallback aliases,
  - explicit review of active FB-* entries,
  - updated sunset decision for any fallback that survives migration phases.
