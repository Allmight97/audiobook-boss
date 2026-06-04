# Decisions

## 2026-06-03 - AAXClean AAX/AAXC Materializer

- Bundle a `.NET 8` AAXClean sidecar as the first Audible AAX/AAXC materializer; `RemoteSourceRuntime` owns helper invocation, provider-secret containment, staged protected files, cancellation, cleanup, and final M4B validation.
- Keep ABB's top-level Apache license unchanged in this workblock while carrying helper source, GPL notices, and public-release compliance notes for the AAXClean helper.
- Guardrail: Dash/Widevine remains unsupported until ABB owns a separate CDM, MPD/PSSH, challenge/response, and content-key selection design.

## 2026-06-01 - Custom Runner Ablation

- The custom verification runner and aliases were removed from current repo guidance.
- Agents use direct Cargo/Nextest/Vitest/build/check commands and record gaps in #341 before adding replacement infrastructure.
- Low-value runner-era scripts were deleted; their claimed protections are recorded in the external ablation report and PR body.
- Guardrail: future AX helpers must prove value over native tool output before becoming repo-local scripts.

## 2026-06-01 - Boundary-Aligned Rust Core Testing

- Focused Rust tests target boundary-aligned workspace packages before the Tauri/media crate.
- `src-tauri` is the runtime, IPC, filesystem, keychain, FFmpeg/audio, and integration shell.
- Core crates must not depend on Tauri, FFmpeg, keyring, or Tauri plugins.
- Use one package/target-selected Nextest command at a time for broad Rust
  review: each core crate, `audiobook-boss --lib`, and
  `audiobook-boss --test all_tests`.
- Committed/generated audio media fixtures are not part of the current test suite.
- Media execution tests remain absent pending issue #341 reassessment. Do not add
  FFmpeg/audio/container tests back until behavior, fixtures, runtime cost, and
  owner boundary are redesigned explicitly.

## 2026-05-31 - Remote Source Acquisition Experiment

- Add `RemoteSourceRuntime` as the eighth Grey-Box Public API for provider-neutral acquisition, backend-only auth/secrets, staged materialized files, Supplemental Assets, and purge behavior.
- Audible provider work may evaluate materializer dependencies, helper binaries, ports, reference implementations, or replacement implementations such as AAXClean. The selected shape requires an explicit design/licensing decision, including ownership, license posture, and distribution implications; do not accidentally absorb third-party implementation code, expose frontend credentials, or fake materialization success.
- Supplemental PDFs attach to imported file-list `inputId`s and are copied only after matching final batch M4B success through output-artifact-owned naming/collision behavior.
- Guardrail: provider-private failures surface as typed diagnostics or failed acquisition status, not silent fallback.

## 2026-05-28 - Bun Stable Runtime

- Use Bun `1.3.14` stable via `packageManager`.
- Refresh: `bun upgrade --stable`.
- Validate Bun changes with direct test/build commands; use `.agents/skills/release` for packaging/release work.
- Script tests run through Vitest via `bun run test`.

## 2026-05-27 - Metadata Intent Validation

- Rust owns metadata intent validation and returns field errors as data for UI preflight.
- TypeScript compiles explicit `set | clear | noop` intent.
- Publication-date normalization and series/subseries slash rejection stay out of TypeScript.
- Output-preview warning validation is non-blocking; save/process workflows still block on validation before persisting or executing metadata intent.

## 2026-05-26 - App Settings And Concurrency

- Serialize settings writes inside `app_settings`.
- On settings update/reset failure, roll back live `JobRegistry` concurrency, not stale persisted preference.
- Hydrate settings per UI owner; one owner failure must not block other owners.
- If settings acceptance succeeds but `getMaxConcurrentJobs` fails, keep accepted UI state.
- Guardrail: do not widen settings patches into cross-registry filesystem transactions without a separate design.

## 2026-05-27 - Metadata/Audio Dependency Scope

- Update only direct dependencies with a concrete trigger in the touched ownership path.
- Leave unrelated Tauri, frontend, and Cargo lockfile churn out of focused dependency releases.
- Guardrail: release verification scope should not expand without a security, compatibility, or release-surface reason.

## 2026-05-27 - Runtime Settings Capability

- Runtime settings capability is one Tauri boundary adapter.
- UI controls do not own independent encoder/concurrency accept/reject tables.
- App Settings stores preferences; Audio owns encoder validity; JobRegistry owns concurrency bounds.
