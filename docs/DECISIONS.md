# Decisions

## 2026-06-26 - Backend Is the Single Source of Processing Lifecycle Truth (PR-4 / #376)

- Outcome: the backend owns processing progress, terminal outcome, and cancellation
  truth for every processing-shaped operation; the UI renders it and never
  re-derives, re-classifies, or multiplexes. Closed in one PR: removed the
  `operation_id` foreground/background discriminator + the background double-emit
  (background ops emit WorkRuntime snapshots only); exported `RunTerminalClass`
  (`abb-processing-core`) onto `ProcessCommandResult`; retired foreground
  `cancel_processing` (cancellation is operation-scoped via `cancel_work_operation`);
  moved `save_metadata_batch` to a WorkRuntime `MetadataSave` operation (Work Center
  renders it) while keeping its synchronous `MetadataSaveBatchResult`; replaced the
  Status Panel's TS terminal-precedence re-derivation with `RunTerminalClass`.
- Evidence: `src-tauri/src/work_runtime/`, `commands/metadata/save_batch.rs`,
  `crates/abb-processing-core/src/lib.rs` (`classify_run_terminal`),
  `src/ui/statusPanel/domain/stateMachineHelpers.ts` (`feedbackFromResult`).
  Reinforces the 2026-06-07 terminal-truth decision (single `classify_run_terminal`).
- Intentional alignment: the backend classifies `success + skipped` as `mixed` (not
  `success`); the preview toast now follows that verdict. Named here so it is read as
  backend-truth alignment, not a UI regression.
- Scoping note: step 6 narrowed the roadmap's deletion bar to terminal
  **re-classification** only — preview reducer state (`jobProgress`/`queueOrder`/
  `latestProgressEvent`) and failure-detail text (`summarizeBatchOutcome`) remain
  where they render backend-sourced state rather than re-deciding the outcome.
- Guardrail: preview stays an **ephemeral render lane** (Inspect/Decide-stage,
  throwaway, no artifact truth). Do not model preview as a Work Center row — that
  would overload the Work Operation invariant; and do not reintroduce a second UI
  terminal classifier.

## 2026-06-24 - Skills vs AGENTS: Location-Bound Knowledge Consolidated

- Discriminator: location-bound knowledge (needed whenever editing a directory) belongs in that
  directory's `AGENTS.md`, which auto-loads by location and cannot miss-trigger; task-bound
  procedures (occasional, cross-cutting) stay skills, which load by intent/invocation.
- Purged four skills whose content duplicated the owning `AGENTS.md`: `job-registry-and-progress`,
  `path-security-validation`, `contract-guardrails` (unique bits relocated to
  `processing/AGENTS.md`, `commands/AGENTS.md`, `scripts/AGENTS.md`), and `testing-strategy` (tier
  tree inlined into root `AGENTS.md`). Kept `release`, `decision-alignment`,
  `resource-lifetime-audit`, `abb-library-research`, and a thinned `audiobook-metadata`. 9 → 5.
- Guardrail: a skill that duplicates a directory's `AGENTS.md` is dead weight that rarely fires —
  prefer the nested `AGENTS.md` for location-bound truth.

## 2026-06-24 - AGENTS Public-Surface List Policy

- Nested `AGENTS.md` files keep an enumerated Public API Strip export list only
  where no test guards the surface. Where a contract test pins the runtime exports
  (`src/lib/tauri`, and `src/ui/{fileList,outputPanel,statusPanel}` via
  `__tests__/runtime-api-contract.test.ts`), the doc points at `index.ts` + that
  test instead of restating symbols.
- Alternatives worth revisiting later: pointerize every surface once each owner has
  a guarding contract test; or generate the lists from `index.ts`/bindings so docs
  cannot drift at all.
- Guardrail: a hand-listed surface that disagrees with `index.ts` or its contract
  test is worse than no list — prefer the test as the source of truth.

## 2026-06-24 - AGENTS Always-Loaded Prune And Lint Enforcement

- Style and stale-cleanup guidance (unused symbols, formatting, `any`) moved out of
  always-loaded `AGENTS.md` prose to deterministic tools the docs point at: Biome
  (`lint:check`), `tsc` (`typecheck`), `cargo clippy`, and new `svelte-check`
  (`check:svelte`) for the previously unlinted `.svelte` surface.
- Rust lint posture is centralized in root `Cargo.toml` `[workspace.lints]` at
  warn-level (members opt in with `[lints] workspace = true`); the CI gate is
  `cargo clippy --workspace --all-targets -- -D warnings`. Warn-level avoids gating
  on `src-tauri`'s GUI-toolkit build, which not every environment can compile.
- Root `AGENTS.md` keeps only architectural, operational, or protective lines;
  agent-temperament and operator-preference prose was removed. Three composition-only
  UI shells merged into `src/ui/AGENTS.md`; the Effect service-catalog table became a
  search instruction.
- Guardrail: do not reintroduce linter-replaceable cleanup prose into the
  always-loaded chain; point at the deterministic tool instead.

## 2026-06-23 - Remote-Source Secret Vault Stays On The Legacy File Keychain

- ABB persists remote-source secrets via `keyring-core` + `apple-native-keyring-store`'s default `keychain::Store` (legacy file keychain, `SecKeychainFindGenericPassword`), service `audiobook-boss.remote-source`, because the Data Protection Keychain (`protected` module) requires Developer ID code signing and a `keychain-access-groups` entitlement the unsigned build does not carry.
- The auth persistence write path is a vault seam in `providers/audible/mod.rs` (`complete_auth` -> `persist_auth`), mock-tested via `MockSecretVault` without a live `register()` exchange.
- Migration to the Data Protection Keychain is deferred to #396, gated on signing/entitlement readiness.
- Guardrail: do not switch `vault.rs` to the `protected` store or add a silent legacy fallback before #396; a vault `Err` propagates as a typed error, never `NeedsAuth` or an empty result.

## 2026-06-11 - Metadata Lookup Provider Degradation Is Canonical

- Audnexus ASIN detail failure continuing to text search, partial multi-provider
  results, and Audible-only provenance when Audnexus detail enrichment fails are
  canonical lookup contract behavior.
- Typed diagnostics in `MetadataLookupResponse` are the observable signal;
  behavior is documented in `src-tauri/src/commands/metadata_lookup/service.rs`.
- Guardrail: external provider partial failure stays explicit in the owning
  command; do not hide degradation or move substitute behavior to callers.

## 2026-06-07 - WorkRuntime Backend Terminal Truth Is Canonical

- WorkRuntime derives operation terminal status from the single
  `abb_processing_core::classify_run_terminal` classifier; the parallel
  `status_from_terminal_summary` rule was deleted (it diverged on success+skipped).
- `classify_terminal_statuses` and `classify_run_terminal` share one rule via
  `RunTerminalClassifier`, so the status-iterator and count-summary paths cannot drift.
- `JobState` collapsed to the registry map-presence model: a tracked job is active,
  and `complete_job`/`fail_job` remove it. Evidence: `src-tauri/src/work_runtime/terminal.rs`,
  `crates/abb-processing-core/src/lib.rs`, `src-tauri/src/processing/job_registry/`.
- Guardrail: do not reintroduce a count-based terminal-classification rule in WorkRuntime;
  map the canonical `RunTerminalClass` to `WorkOperationStatus` instead.

## 2026-06-03 - AAXClean AAX/AAXC Materializer

- Bundle a `.NET 8` AAXClean sidecar as the first Audible AAX/AAXC materializer; `RemoteSourceRuntime` owns helper invocation, provider-secret containment, staged protected files, cancellation, cleanup, and final M4B validation.
- Keep raw Tauri dev/build routes and release build scripts helper-aware by publishing the sidecar before runtime resolution or packaging.
- Keep ABB's top-level Apache license unchanged while carrying helper source, GPL notices, and public-release compliance notes for the AAXClean helper.
- Guardrail: Dash/Widevine remains unsupported until ABB owns a separate CDM, MPD/PSSH, challenge/response, and content-key selection design.

## 2026-06-01 - Custom Runner Ablation

- The custom verification runner and aliases were removed from current repo guidance.
- Agents use direct Cargo/Nextest/Vitest/build/check commands and record gaps in #341 before adding replacement infrastructure.
- Deleted runner-era scripts must not be recreated without measured value over direct native tool output.
- Guardrail: future AX helpers must prove value over native tool output before becoming repo-local scripts.

## 2026-06-01 - Boundary-Aligned Rust Core Testing

- Focused Rust tests target boundary-aligned workspace packages before the Tauri/media crate.
- `src-tauri` is the runtime, IPC, filesystem, keychain, FFmpeg/audio, and integration shell.
- Core crates must not depend on Tauri, FFmpeg, keyring/credential-store crates, or Tauri plugins.
- Use one package/target-selected Nextest command at a time for broad Rust
  review: each core crate, `audiobook-boss --lib`, and
  `audiobook-boss --test all_tests`.
- Committed/generated audio media fixtures are not part of the current test suite.
- Media execution tests remain absent pending issue #341 reassessment. Do not add
  FFmpeg/audio/container tests back until behavior, fixtures, runtime cost, and
  owner boundary are redesigned explicitly.

## 2026-05-31 - Remote Source Acquisition Runtime

- Add `RemoteSourceRuntime` as the eighth Grey-Box Public API for provider-neutral acquisition, backend-only auth/secrets, staged materialized files, Supplemental Assets, and purge behavior.
- Audible provider work may evaluate materializer dependencies, helper binaries, ports, reference implementations, or replacement implementations such as AAXClean. The selected shape requires an explicit design/licensing decision, including ownership, license posture, and distribution implications; do not accidentally absorb third-party implementation code, expose frontend credentials, or fake materialization success.
- Supplemental PDFs attach to imported file-list `inputId`s and are copied only after matching final batch M4B success through output-artifact-owned naming/collision behavior.
- Guardrail: provider-private failures surface as typed diagnostics or failed acquisition status, not silent or undeclared degradation.

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
