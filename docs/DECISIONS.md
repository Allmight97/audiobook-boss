# Decisions

## 2026-06-01 - Boundary-Aligned Rust Core Proof

- Focused Rust proof targets boundary-aligned workspace packages before the Tauri/media crate.
- `src-tauri` is the runtime, IPC, filesystem, keychain, FFmpeg/audio, and integration shell.
- Core crates must not depend on Tauri, FFmpeg, keyring, or Tauri plugins.
- `review rust` uses Nextest for core crates plus non-media runtime shell tests;
  `review core` is the pure fast domain route.
- Committed/generated audio media fixtures are not part of canonical proof or
  the current test suite. Existing media execution tests were purged pending
  issue #341 reassessment.
- Media execution proof is suspended pending issue #341 reassessment. Do not add
  FFmpeg/audio/container tests back to canonical proof until the behavior,
  fixtures, runtime cost, and owner boundary are redesigned explicitly.

## 2026-05-30 - Proof Evidence Retention

- Proof output is terminal-first.
- Successful proof runs discard temp logs.
- Failed proof runs keep OS-temp evidence and print the artifact directory and summary path.
- `bun run proof:clean` is only for legacy repo-local `.proof/` cleanup.

## 2026-05-28 - Bun Stable Runtime

- Use Bun `1.3.14` stable via `packageManager`.
- Refresh: `bun upgrade --stable`.
- Bun-change proof: `bun scripts/proof/runner.ts review`; use `release` for packaging/release work.
- Script proof tests: Vitest via `bun run test`.

## 2026-05-27 - Proof Entrypoint

- Use `bun scripts/proof/runner.ts`; public categories are `focus`, `review`, `release`, and `diagnose`.
- Full Rust review proof uses `cargo nextest run`; Cargo remains the Rust build/compiler executor.
- Focused Rust routes must stay target-aware: `--lib` for library-owned tests, consolidated `--test all_tests <module>[::filter]` for integration modules.
- Trigger: issue #349 measurements showed structured reporting and broad Rust test execution became repeated proof pain.

## 2026-05-27 - Proof Orchestration

- Canonical proof runner is Bun under `scripts/proof/`.
- Do not add root `.mise.toml` as a replacement proof gate unless it delegates to the Bun runner.
- Guardrail: sequential review gate semantics are not equivalent to parallel mise sibling tasks.

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
- Guardrail: release proof scope should not expand without a security, compatibility, or release-surface reason.

## 2026-05-27 - Runtime Settings Capability

- Runtime settings capability is one Tauri boundary adapter.
- UI controls do not own independent encoder/concurrency accept/reject tables.
- App Settings stores preferences; Audio owns encoder validity; JobRegistry owns concurrency bounds.
