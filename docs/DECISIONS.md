# Decisions

## 2026-08-29 - Solid Renderer And Effect Atom Runtime (#468)

- Outcome: Solid is ABB's only desktop renderer. Solid owns screen-local
  interaction; owner view atoms carry durable cross-component state; Effect
  owns effectful workflows; one app-lifetime registry composes and disposes
  the frontend runtime.
- Evidence: `package.json`, `src/main.tsx`, `src/app/runtime/`, and
  `scripts/frontend-toolchain-layout.test.ts`.
- Guardrail: workflow Effect, unstable reactivity, and the Solid binding enter
  through their three ABB seams; renderers consume owner Public API Strips
  rather than writable private atoms.

## 2026-08-27 - TypeScript 7 / Effect 4 Frontend Toolchain (#462)

- Outcome: ABB's frontend compiler is TypeScript 7 via `@typescript/native`
  and the `typescript` package slot. ABB-owned code does not import
  `typescript`. Bun is `1.4.0`. Effect is an exact v4 RC pin
  (`4.0.0-rc.112` at landing), not a range. Workflow owners consume Effect
  only through `src/lib/effect/appEffect.ts`.
- Evidence: `package.json`, `bun.lock`, `.github/workflows/ci.yml`,
  `scripts/check-tauri-runtime-boundary.ts`.
- Guardrail: do not reintroduce a TypeScript 6 shim or `svelte-check`. The
  runtime-boundary check is a text scan, not an AST parser. Do not grow it
  toward AST completeness. Replace it when TypeScript 7.1's programmatic API
  exists and a focused owner needs that parser.

## 2026-08-27 - Dependency Resolution And Release-Age Policy

- Outcome: `Cargo.lock` and `bun.lock` are committed resolution truth; any CI
  or verification command that resolves dependencies uses frozen/locked
  installs. Manifest ranges remain compatible by default. Exact pins are
  reserved for proven prerelease families,
  cross-version type boundaries, synchronized families, and vendored or
  provenance-sensitive dependencies.
- Outcome: a blanket 10-day minimum release age applies to every ordinary
  dependency update, no family tiers. It is mechanical on both surfaces:
  `bunfig.toml` `minimumReleaseAge` gates fresh Bun resolutions and the
  Dependabot cooldown gates update PRs. Cargo has no release-age filter, so
  manual `cargo update` has no gate and relies on reviewer discipline.
  Security fixes bypass the age with focused proof.
- Outcome: weekly Dependabot version updates for Cargo and the text `bun.lock`
  ecosystem use the same 10-day cooldown, compatible-update groups, and a low
  PR limit.
- Grandfather: the 2026-08-27 refresh (h2 security fix plus blanket
  `cargo update`/same-major Bun refresh) predates this policy and is its
  baseline; do not re-audit its individual package ages.
- Guardrail: retain the split reqwest 0.12 Audible boundary, exact Specta RC
  pins, Tauri API override, and vendored FFmpeg provenance. RustCrypto majors
  and TypeScript 7/jsdom 30/jest-dom 7 are dedicated migrations, not routine
  refreshes. Evidence: `Cargo.toml`,
  `src-tauri/Cargo.toml`, `package.json`, `bunfig.toml`, and
  `.github/dependabot.yml`.

## 2026-08-24 - Function Complexity Is An Attention Ratchet, Not A Gate (#454)

- Outcome: CCN >8 stays an attention prompt adjudicated by issue #454's four
  questions (consequence, proof, structure, change pressure). New or reshaped
  functions target roughly CCN ≤10 / cognitive ≤15, with named exceptions for
  dispatch `match`/`switch` and sequential `?` lifecycle chains. The existing
  high-CCN findings are grandfathered: adjudicate each at its next change
  point; coverage decides sequencing (pin behavior, then reshape), never
  whether a function is worth reducing.
- Evidence: source audit of the top five #454 candidates found Rust `?`
  inflates CCN on cohesive orchestrators (`materialize`, `setup_encoder`,
  `rewrite_metadata_with_ffmpeg_plan_as`) whose splits would scatter RAII and
  ordering invariants, while `readMetadataForm` (CCN 38) is duplicated control
  flow over the existing `METADATA_FIELD_DEFINITIONS` table; the threshold
  literature defends 10/15 as testability convention, not a measured defect
  cliff (McCabe 1976, NIST SP 500-235, Shepperd 1988, El Emam 2001).
- Guardrail: no repo-wide CCN reduction campaign; the new-code threshold
  loosens only by explicit owner decision — agents do not move it.

## 2026-08-13 - Committed Upstream Research Snapshots Retired

- Outcome: committed upstream research snapshots were retired.
  External-library research uses resolved lockfile versions, installed or
  registry-packaged source, Context7, exact public package docs, and ephemeral
  verified source retrieval.
- Evidence: the snapshots tracked stale upstream branches, produced hundreds of
  Dependabot alerts against research lockfiles, and exceeded GitHub's default
  150-manifest dependency-graph processing budget.
- Guardrail: do not recommit general research clones; keep `vendor/ffmpeg-sys-next`
  as build provenance, not research material.

## 2026-08-11 - Hosted Proof Is One Clean-Frontend Alarm (#450)

- Outcome: GitHub automatically runs only Pages publication and a
  path-narrowed frontend frozen-install/typecheck alarm. Rust tests and
  generated-binding verification stay explicit local or release checks.
- Evidence: the clean frontend runner caught an undeclared `@types/node`
  dependency hidden by a warm checkout; hosted core tests had no unique catch,
  and the binding job's only catch was a trailing newline after compiling the
  FFmpeg-linked runtime.
- Guardrail: automatic hosted work needs a demonstrated clean-runner or
  publication outcome that local/release proof does not already own.

## 2026-08-10 - Specta RC25 Keeps ABB's Numeric IPC Contract (#448)

- Outcome: ABB adopts Specta/tauri-specta RC25 and keeps bounded byte sizes,
  timestamps, counts, indices, and sequence values as TypeScript `number` at
  the Tauri IPC boundary. The exporter opts into the RC25 integer remapper at
  that boundary; pure domain crates retain their Rust-owned integer types.
  Float fields used for progress and file totals stay numeric only where their
  Rust owners normalize non-finite values before serialization.
- Evidence: RC25 binding generation rejects wide integers without an explicit
  policy and represents potentially non-finite floats as `number | null`.
  `src-tauri/src/ipc_contract.rs` owns the integer policy;
  `src/lib/tauri-client.generated-event-bindings.test.ts` pins representative
  numeric shapes; focused Rust tests pin non-finite progress normalization.
- Guardrail: do not introduce JavaScript `bigint` or lossless-float semantic
  transforms without a concrete payload that needs those runtime semantics.
  An owner-local `specta-typescript::Number` annotation is allowed only when
  the Rust owner normalizes the value to a finite number and the established
  wire contract must remain non-nullable; keep focused owner and generated-type
  proof beside that exception.

## 2026-08-09 - FFmpeg 9 Uses a Minimal Source-Provenance Vendor (#441)

- Outcome: ABB moves `ffmpeg-next` and `ffmpeg-sys-next` together to 9.0.0 and
  retains a minimal `ffmpeg-sys-next` vendor to replace its mutable
  `release/9.0` clone with FFmpeg tag `n9.0`, verified at peeled commit
  `d32b387f2b0a484599d4587d651891f0c63c4238`, and restore the `CoreAudio`
  framework required by FFmpeg's AudioToolbox device symbols. Distributed
  DMG builds enable `build-portable`; ordinary source builds target their
  compiling Apple Silicon host natively. No other Apple behavior is carried.
- Evidence: exact v1.3.1 source compiled unchanged against Homebrew FFmpeg 9
  and completed an upstream bundled FFmpeg 9 build; upstream sys 9.0.0 still
  follows a mutable release branch. The first bundled runtime-test link then
  failed on `AudioObjectGetPropertyData*` until `CoreAudio` was restored.
- Guardrail: keep the vendor diff limited to source selection, commit
  verification, and the proven `CoreAudio` link requirement; advance the
  FFmpeg tag/commit deliberately with wrapper/sys review and the media,
  packaging, and release proof in issue #441. Only builds producing a
  distributable DMG use `bundled-ffmpeg-portable`; local development, tests,
  app builds, and developer installs use `bundled-ffmpeg`.

## 2026-07-02 - Artifact Drawer Removed; Clear Path Stays Contractual

- Outcome: the "Embedded artifacts" inspect/clear drawer (1.3.0, #281) was
  removed from the Metadata Manager as UX load without clear value in that
  form; re-ideation captured in #411. The backend artifact clear path,
  `stageMetadataIntentPatch` explicit clears, the draft-field exclusion, and
  the media-execution round-trip proof are all retained and pinned by the
  Metadata Session contract test.
- Evidence: `src/ui/metadataSession/__tests__/runtime-api-contract.test.ts`
  (artifact preservation + explicit clears); `src/ui/metadataArtifacts/`
  deleted.
- Guardrail: artifact fields never enter `METADATA_DRAFT_FIELDS`; any future
  artifact surface reuses the Metadata Session clear path rather than new
  staging mechanics.

## 2026-07-02 - Toolchain Platform-Probe Seam

- Outcome: FFmpeg candidate enumeration and binary-arch acceptance are
  per-platform cfg-dispatched functions in
  `src-tauri/src/audio/toolchain/platform.rs` (vault.rs pattern: per-OS fn +
  explicit unsupported fallback). macOS (Homebrew prefixes, arm64/arm64e) and
  Linux (Mint-class `/usr` prefixes, ELF x86-64) probes exist; Windows is
  deliberately absent until that port. The `crate::audio` Public API Strip is
  unchanged; platform ordering/acceptance rules are pure functions with
  filesystem canonicalization injected, so their unit-test proof is
  host-independent (tests stub the canonicalizer; production collapses
  symlinked candidates).
- Evidence: `src-tauri/src/audio/toolchain/platform.rs` (rules + tests);
  real-Linux runtime proof (actual `file`/`pkg-config`/apt ffmpeg) is
  deferred to the Linux port — this slice proves the rules, not the runtime.
- Guardrail: platform-specific paths, prefixes, and arch rules live only in
  `platform.rs`; resolution and codec validation stay platform-neutral in
  `toolchain/mod.rs`.

## 2026-07-02 - metadataSession Owner Strip

- Outcome: frontend metadata cache/pending-intent/validation/save truth
  consolidated under `src/ui/metadataSession` (absorbs the six `src/ui` root
  loose files + `src/ui/core/`); outcome-shaped strip (30 scattered exports →
  12) pinned by `__tests__/runtime-api-contract.test.ts`. `tagPreview.ts` and
  `metadataLookup.ts` folded into their own owners' `index.ts` ("three
  destinies"); tagPreview's reach into metadataForm private preview state
  replaced by the public `readMetadataFormPreviewValues()` accessor.
- Evidence: `src/ui/metadataSession/index.ts`,
  `src/ui/__tests__/metadata-session-smoke.test.ts` (edit→save and
  lookup-apply→save through real session state).
- Guardrail: pending markers are created only via `stageMetadataIntentPatch`
  (save workflow clears on success; lifecycle clears via
  `removeMetadataForFile`/`clearMetadataSession`); no caller-side
  merge/equality staging logic returns. Live workflow services on the
  fileList/statusPanel cycles capture imports lazily, never by value.

## 2026-07-01 - Pre-Marketing Posture Decisions (#406 / #407 closeout)

- Outcome: current local and GitHub DMG distribution remains unsigned. Public
  releases use the verified portable Apple Silicon artifact route; signing,
  notarization, the `keychain-access-groups` entitlement, and #396 (Data
  Protection Keychain migration) remain deferred until the owner explicitly
  adopts a signed distribution lane. No keychain code changes before that
  posture changes.
- Dependency/supply-chain cadence (#326): weekly Dependabot/manual review is
  the routine lane, with a blanket 10-day release age (see 2026-08-27).
  `bunfig.toml` `minimumReleaseAge` remains the standing Bun resolution guard;
  security fixes bypass the age with focused proof.
- Effect workflow kit (#389): spike accepted, rolled out to all five workflow
  owners (`makeWorkflowKit`); convention recorded in `src/lib/effect/AGENTS.md`.
- #180 perf attribution stays open as the sole post-launch research issue;
  measurement-first, no perf refactors without attribution evidence.
- Guardrail: opencode reviewer workflow removed (no key, zero successful
  runs); restore from git history only with a working OpenCode API key.

## 2026-07-01 - Media Execution Lane Added; Narrow Tripwire CI (#341 / #407)

- Outcome: media-execution route decided as `add`. The smallest maintained
  real-media lane lives in
  `src-tauri/tests/cases/integration_media_execution_tests.rs`: WAV fixtures
  synthesized at test time (no committed media), native in-process ffmpeg-next
  path, headless `ProcessingContext`. Proves import→process→valid M4B +
  truthful duration, metadata save→re-read from the artifact, and
  cancellation→typed terminal error with no artifact/staging residue. It runs
  in the normal `audiobook-boss --test all_tests` suite (~1s).
- Evidence: the lane's first run caught a real user-facing bug — WAV inputs
  failed native processing ("Resample failed: Input changed") because
  layout-less PCM containers open with an unspecified channel layout; fixed at
  the owning boundary in `src-tauri/src/audio/processor/streams.rs`.
- CI posture: one narrow tripwire workflow (`.github/workflows/ci.yml`) on
  push to `main` — frozen install + typecheck, generated-binding
  drift, core-crate Nextest. Deliberately not a broad app gate; local checks
  remain the default evidence trail. The opencode reviewer workflow was
  removed (no API key, zero successful runs).
- Guardrail: never commit media fixtures; keep the lane's runtime budget ~1s
  by shrinking fixtures, not by weakening assertions. Widen CI only with a
  recorded owner decision.

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

## 2026-07-04 - Encoder Settings Carry No Inert Knobs

- `EncoderSettings` has no `threads` or `twoloop` field: AAC encoders (native
  `aac`, `aac_at`, `libfdk_aac`) do not frame-thread, and FFmpeg's native
  coder default is already twoloop — both knobs were end-to-end no-ops.
- The in-process engine refuses `FdkHeAac` with a typed error; FDK is owned
  exclusively by the external FFmpeg adapter (evidence: adapter routing in
  `processor/adapter.rs`, encoder guard in `processor/encoder/context.rs`).
- Guardrail: reintroducing an encoder knob requires evidence it changes output
  for at least one shipped encoder path.

## 2026-07-04 - Resampler Tail Is Bounded And Flushed At File Boundaries

- The frame pipeline sizes resampler output frames from current swr delay plus
  input samples scaled to the output rate, then streams EOF flush frames through
  the accumulator/encoder; with rate conversion active, skipping the flush
  silently drops end-of-file audio.
- Evidence: inline swr-tail and upsample backlog tests in
  `frame_pipeline.rs` plus the rate-converted media-lane test.
- Guardrail: any new resample site must either reuse this pipeline or both size
  output buffers from swr delay and flush its own swr delay before reporting a
  file complete.

## 2026-07-04 - MP4 Artifact Tags Must Not Depend On Bare Remux

- External FDK finalization remuxes for chapters/cover, then rewrites
  MP4-family tag truth through mp4ameta. Native finalization already muxes
  chapters/cover in-process and keeps its direct mp4ameta metadata write.
- Evidence: the FFmpeg mov muxer drops dict keys outside its known-atom table
  (`series`, `series-part`, iTunes freeform mirrors, `sort_album`), so the
  external FDK adapter's remux-only finalize lost those tags; pinned by
  `artifact_finalize_preserves_series_tags_and_chapters_on_mp4_route`
  (ABB readback + external `ffprobe` proof).
- Guardrail: an encoder path may not write MP4-family artifact tag truth through
  the FFmpeg dictionary alone.

## 2026-07-04 - Series Edits Validate Effective Round-Trip Shape

- Series/subseries writes preserve representable partials (`Series; Subseries`
  without parts), but reject touched orphan shapes that cannot round-trip
  (`series-part` without `series`, or `subseries-part` without the complete
  primary series + primary part + subseries chain).
- Evidence: `abb-metadata-core` source-aware write-plan tests and
  `metadata_ops` partial-subseries field-op proof.
- Guardrail: inherited odd tags from external files do not block unrelated
  saves or processing; strict shape validation applies when intent touches the
  series family.

## 2026-08-09 - FileList Sort Changes Processing Order

- Outcome: filename sorting rewrites the queue using natural numeric basename order; FileList retains path-keyed arrival ordinals and exposes Restore import order when visible order diverges.
- Evidence: `src/ui/fileList/actions.ts` plus reorder, restore-order, selection, and payload-order tests.
- Guardrail: selection follows file identity, manual reorder clears the sort claim, and `aria-sort` reflects the actual queue order.

## 2026-08-09 - Frontend Failures Enter Local Dev Logs Through One Boundary

- Outcome: webview errors and unhandled rejections reach the local captured dev log through the bounded `log_frontend` Runtime Boundary command.
- Evidence: `src/lib/frontendLogBridge.ts`, `src-tauri/src/commands/frontend_log.rs`, and `scripts/dev-log-analysis.test.ts`.
- Guardrail: forward only a short sanitized error name/category and message; arbitrary rejection values, provider payloads, and secrets do not cross the boundary.
