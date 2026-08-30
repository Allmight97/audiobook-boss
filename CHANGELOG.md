# Changelog

All notable changes to AudioBook Boss™ will be documented in this file.

## [Unreleased]

## [1.3.6] - 2026-08-28

### Changed

- Updated the frontend toolchain to TypeScript 7 and Effect 4 RC, with Bun
  1.4.0 required for the supported frontend diagnostic setup. This is a patch
  release because the migration preserves ABB's existing user-facing feature
  and IPC behavior while improving dependency and runtime foundations.

### Fixed

- Kept QuickTime chapter timestamps valid when cover art is embedded in an
  audiobook.
- Reported an unavailable external FDK encoder when an FFmpeg candidate cannot
  start, instead of treating the candidate as ready.

## [1.3.5] - 2026-08-13

### Changed

- Removed committed upstream research snapshots from the repository dependency
  graph. External-library research now uses ABB's resolved package versions,
  installed or registry source, exact public docs, and ephemeral verified
  upstream retrieval only when necessary.
- Updated the Rust property-list/XML dependency chain and frontend build
  toolchain to patched releases. Dependency auditing now checks both Rust and
  JavaScript graphs without one failed audit suppressing the other.

## [1.3.4] - 2026-08-13

### Changed

- Release packaging is now deterministic: Cargo binary discovery is explicit,
  bundle verification rejects any unexpected executable next to the app and
  AAXClean helper, and both shipped executables are checked for architecture
  and external library links.
- Public DMG builds always rebuild the AAXClean helper from current source
  instead of reusing machine-history output; developer builds keep fast reuse.
  A failed or interrupted helper publish no longer copies leftover output.
- Formal releases now verify the uploaded GitHub asset is byte-for-byte the
  locally validated DMG (`gh release verify-asset`).

## [1.3.3] - 2026-08-10

### Changed

- Updated ABB's Tauri desktop runtime, build tooling, and dialog stack to the
  current compatible patch releases.
- Updated the Specta/tauri-specta binding stack to RC25 and made ABB's numeric
  IPC contract explicit for bounded sizes, timestamps, counts, indices,
  sequence values, and progress values.

### Fixed

- Non-finite progress values are normalized at their Rust owners instead of
  becoming unexpected `null` values in generated JSON payloads.
- Work Center source files and generated previews can open again from
  user-owned, temporary, and mounted-volume paths. Opener failures now appear
  as Work Center feedback instead of an unhandled frontend rejection.
- Public and local app bundles no longer include developer-only TypeScript
  binding-export or AAC decoder-contract executables.

## [1.3.2] - 2026-08-10

### Changed

- Migrated ABB's in-process media stack and portable bundled builds to FFmpeg
  9.0 with a reproducibly pinned upstream source; existing audio, metadata,
  output, IPC, and user-supplied external-FDK behavior remains unchanged (#441).
- Source-built apps now target the compiling Apple Silicon Mac natively;
  prebuilt DMG releases retain a portable Apple Silicon baseline.

## [1.3.1] - 2026-08-09

### Added

- Design-system foundations ahead of the UI evolution (#412): a full token
  scale (spacing, type, radius, motion) with a user-preference density switch
  (`data-density`), a shared primitive kit (cover thumbnail, modal
  status/empty, progress track), and a dev-only design lab at `/lab.html`
  rendering every token and primitive for visual review. Progress bars across
  Status Panel, Work Center, and Remote Source now share one chosen height.
  Interactive UI direction mockups stored under `docs/design/`.
- Codex agent environment setup script for ABB worktrees/cloud sessions:
  installs the Linux/Tauri media-lane prerequisites, builds the pinned FFmpeg
  major with `libmp3lame`, creates the gitignored AAXClean sidecar stub, and
  documents the Linux-vs-macOS proof boundary for encoder-path testing.

### Changed

- Find Metadata Online now shows separate Title and Author search fields,
  auto-filled from the selected file (title/album into Title, author into
  Author), so you can see and adjust each criterion before searching. ASIN
  pastes into the Title field keep working.
- Frontend metadata truth (per-file cache, pending drafts, validation, and
  batch save) is consolidated under one owned Metadata Session module with a
  contract-pinned surface and an end-to-end edit→save / lookup→save smoke
  test. No user-visible behavior change; re-applying values identical to the
  file now truthfully reports "no pending metadata changes" instead of
  re-writing them.
- FFmpeg toolchain probing is now per-platform behind one seam: macOS
  behavior unchanged; Linux (Mint-class) probe rules ship unit-tested ahead
  of the Linux port. Error messages about Apple AAC availability are
  platform-truthful.
- The media test lane now proves the Apple AAC (AudioToolbox) encoder route
  end-to-end alongside Native AAC — real processing to a valid M4B with
  truthful duration and tag re-read. External FDK real-execution proof stays
  manual or env-gated because it requires a user-supplied `libfdk_aac`
  FFmpeg build.
- Native AAC always uses FFmpeg's best coder (twoloop) — previously a
  "Twoloop" checkbox appeared to control this but had no effect either way.
  Apple AAC now pins its wrapper quality option explicitly at the best value
  (the scale is inverted: 0 is best), guarding against accidental
  degradation (#413).
- The media test lane also covers sample-rate-converted merges and stereo
  channel preservation (per-channel RMS on the decoded artifact), and its
  Apple AAC tests skip cleanly on non-macOS agents (#413).

### Removed

- The "Embedded artifacts" inspect/clear drawer under the metadata form
  (added in 1.3.0). Normal saves still preserve Album sort (TSOA), Comment,
  Track, and Disc untouched — only the inspect/clear surface is gone while
  its purpose is re-evaluated (#411).
- The Native AAC "Twoloop" checkbox and the internal encoder threads
  setting: both were end-to-end no-ops (FFmpeg's coder default is already
  twoloop; AAC encoders do not frame-thread). Saved settings containing the
  old fields load cleanly (#413).

### Fixed

- Rate-converted encodes (output sample rate different from the source) no
  longer drop the resampler's held-back tail at each file boundary — a few
  milliseconds of end-of-file audio per input file — and upsampled inputs
  stream converted audio steadily instead of buffering it until end of file
  (#413).
- Audiobooks encoded with the FDK HE-AAC (external FFmpeg) encoder no longer
  lose series, series number, and album-sort tags on the finished file:
  artifact metadata finalize now rewrites MP4 tag truth through mp4ameta after
  the remux that carries chapters/cover, proven against an external reader
  (`ffprobe`), with chapters preserved.
- Metadata Manager subseries edits no longer disappear unless the full
  series/subseries quadruple is present: representable partials are written to
  artifact tags, while touched orphan shapes fail before processing or save
  instead of silently clearing series tags (#415).
- Merge processing now stages dirty Metadata Manager edits onto the merge
  artifact owner instead of dropping edits made while a non-first input is
  selected; non-merge processing blocks dirty edits on an invalid selection
  instead of retargeting them to another file (#415).

## [1.3.0] - 2026-07-02

### Added

- App Settings panel (Cmd+,) with a durable, validated path to your own
  FFmpeg build with `libfdk_aac` — a working user path unlocks the FDK HE-AAC
  encoder and wins over auto-detection; a rejected path falls back to
  auto-detection with the rejection reason shown, never silently.
- Pinned launch defaults: pin your current encoder, output, and max-jobs
  settings as startup defaults and choose whether launch restores those
  defaults or your last-used settings (the previous behavior, still the
  default). In-flight tweaks stay session-only when pinned mode is on.
- Embedded artifacts drawer under the metadata form: inspect and clear
  Album sort (TSOA), Comment, Track number, and Disc number on the selected
  file. Clears apply on the next metadata save; normal saves keep preserving
  these fields untouched.

### Changed

- If the encrypted Audible source can't be removed after a successful
  decryption, the app now retries briefly and then shows a non-blocking
  notice on the acquisition instead of only logging — the decrypted book
  stays import-ready either way, and leftover files are cleaned on next
  launch.
- When an output destination (network share, external drive, cloud-synced
  folder) refuses the final file commit, the error now says so directly and
  suggests checking write permissions/space or retrying to a local folder,
  instead of reading like a generic processing failure.
- Crash-leftover temporary files beside a destination file are now swept
  automatically the next time the same audiobook is written there.
- The automated test suite now proves real media behavior end-to-end — WAV,
  M4B, and MP3 inputs, cover-art round-trips, chapter preservation, and
  metadata integrity on real artifacts — with all fixtures synthesized at
  test time. A narrow CI tripwire (typecheck, binding drift, core tests) now
  runs on every push to main.

### Fixed

- WAV files failed native processing with "Resample failed: Input changed" —
  containers without channel-layout metadata (WAV/PCM) now normalize to the
  correct layout before resampling, so WAV imports process correctly.

## [1.2.0] - 2026-06-24

### Added

- Remote acquisition now shows cover thumbnails for matched audiobooks, with
  Audible product-image covers mapped into the acquire-dialog preview.

### Changed

- Tightened metadata cover-art compatibility, Audible Supplemental PDF
  acquisition, and Status Panel ownership so format detection, PDF
  orchestration, and UI state updates live with their owning modules.
- Moved native audio processing onto a blocking worker so long encodes do not
  occupy async runtime workers and cancellation stays responsive during
  concurrent processing.
- Removed dead internal seams from the audio processing pipeline — the
  `ProcessorAdapterKind` mirror enum, unused finalize reporter/passthrough
  parameters, the `ProcessingRun` namespace struct, and a stray progress
  reporter — leaving the encode path with less unused indirection. No behavior
  change.
- Consolidated metadata compatibility proof at owning modules and tightened
  encode/acquisition orchestration seams for workspace lint compliance.

### Fixed

- Cover-art compatibility warnings now distinguish supported JPEG/PNG, known
  unsupported formats, and unrecognized bytes without warning on arbitrary
  placeholder data.
- Restored a green runtime test suite by replacing stale cover-art integration
  tests with owner-boundary proofs in the metadata layer and clearing the
  workspace clippy gate introduced in 1.2.0 prep.

### Security

- Updated quinn-proto to 0.11.15, fixing RUSTSEC-2026-0185 (memory exhaustion
  via unbounded out-of-order QUIC stream reassembly) on remote acquisition
  paths.
- Dropped the keyring umbrella crate in favor of registering the native OS
  keychain directly via `keyring-core`, removing the memmap2 advisory
  (RUSTSEC-2026-0186) and its unused database keystore backends. Keychain
  behavior is unchanged.
- Pinned undici to >=7.28.0 to clear transitive jsdom dev-only advisories and
  updated Vite to 8.0.16 plus other build/test dependencies.

## [1.1.4] - 2026-06-15

### Changed

- Tightened path, remote-source, and audio contract hygiene so basename fallback
  behavior is explicit, Status Panel consumes remote-source processing helpers
  through the public UI surface, and external audio tooling preserves OS-native
  path arguments.

### Fixed

- Packaged macOS builds no longer open to a blank window: production frontend
  bundling avoids split-chunk cycles and tauri:// asset loading issues that
  dev mode does not exercise.

## [1.1.3] - 2026-06-11

### Changed

- Tightened file-list, status-panel, metadata-lookup, and remote-source UI
  boundaries so panels consume owner-specific surfaces instead of reaching
  through removed shared helpers.

### Fixed

- Multi-file import no longer skips full metadata for the first selected file
  when a cover-only cache entry was written during import.
- Status step color returns to neutral after informational messages that follow
  an error or success state.
- External OS file drags no longer show false reorder highlights in the file
  list.

## [1.1.2] - 2026-06-10

### Changed

- Tightened metadata passthrough and processing/output-artifact boundaries so
  audio processing uses owner-specific public APIs with focused contract
  coverage.
- Moved FileList rendering and OutputPanel review workflow access behind their
  public UI surfaces, reducing cross-panel state reach-through while preserving
  existing import and processing workflows.

### Fixed

- File-list move and remove buttons no longer also select the clicked row.

## [1.1.1] - 2026-06-09

### Fixed

- Cover art ownership now follows output mode consistently: merge cover updates
  commit to the merge key, batch updates apply only to one selected file, and
  batch multi-select ignores cover-art commits.
- Metadata lookup cover previews now load automatically through a bounded,
  backend-proxied scheduler while preserving URL-keyed cache and stale-result
  guards.
- Audible acquisition validation now moves materialized audio probing and hash
  work off the async executor.

## [1.1.0] - 2026-06-09

### Changed

- Runtime settings capability loading now has explicit cache, refresh, and
  invalidation semantics so App Settings can pass one capability snapshot into
  encoder and job controls.
- Remote acquisition ownership is now split across tested UI selection policy,
  backend runtime lifecycle, provider-private Audible per-title acquisition,
  and output-artifact Supplemental PDF commit boundaries.

### Fixed

- Metadata lookup results no longer render provider-controlled cover URLs
  directly; selected cover art still loads through the backend-validated
  cover-art URL path.
- Pages deployment actions are pinned to audited commit SHAs.
- Required Supplemental PDF output commits now report artifact-owned partial
  failure truth while preserving the created audiobook output.

## [1.0.37] - 2026-06-09

### Changed

- Work Center now uses backend-authored WorkRuntime snapshots as the background
  operation truth, with Status Panel retained for preview, metadata save, and
  direct foreground cancellation.
- Tightened frontend runtime-boundary validation so Tauri command mocks and
  runtime settings capability state fail typecheck when generated IPC shapes
  drift.
- Split processing run, remote-source acquisition dialog, Audible probe, and
  remote-source session lifecycle internals into clearer ownership modules.
- Enhanced `bun run app:dev:log` with run summaries, Tauri package/runtime
  matrix output, port handling, and a separate encoder log.
- Updated Tauri frontend/Rust dependency declarations to the 2.11 runtime
  family resolved by the lockfiles.

### Fixed

- WorkRuntime cancellation now preserves the visible cancelling state when late
  processing progress arrives before terminal cancellation.
- Remote-source logout and cancellation cleanup now purge stale sessions more
  reliably without deleting already materialized handoff sessions.

### Removed

- Removed the stale encoder panel shell, dependency-only Effect smoke test, and
  completed pattern-cleanup roadmap packet.

## [1.0.36] - 2026-06-07

### Changed

- Reworked the authoring workspace into separate metadata, encoding, output,
  tags, input, and file-inspector zones to reduce scrolling and keep `App.svelte`
  as shell composition.
- Updated the default app window to a 1600x1000 logical canvas with a 1440x900
  minimum and monitor-fit startup sizing for high-DPI displays.
- Renamed the main processing action to "Start Processing".

### Fixed

- `bun run app:dev:log` now replaces an existing ABB-owned dev server on port
  1420 instead of failing with a stale-port error.

## [1.0.35] - 2026-06-07

### Added

- Work Runtime: backend owner for accepted operation identity, immutable
  submissions, operation snapshots, operation-scoped cancellation, and terminal
  summaries for batch and merge processing.
- Work Center UI: multiple background operations with child rows, progress,
  operation cancel, terminal summaries, and source actions.
- FileList now unlocks after accepted submit, allowing continued
  editing/importing/reordering while background operations run.
- Encode-permit acquisition with external cancel flag so operation-scoped jobs
  ignore legacy global cancel, including while waiting for permits.
- Output parent directory cleanup tracks only ABB-created empty parent dirs;
  cancelled/failed operations prune those dirs while preserving preexisting,
  non-empty, and symlinked paths.

### Changed

- Final batch/merge processing routes through accepted background submissions
  (Work Runtime); preview processing stays on the legacy foreground path.
- Status Panel ignores WorkRuntime-scoped legacy queue/progress events and
  its Cancel action fans out only to visible foreground job IDs.
- Remote-source sessions are retained while accepted processing operations are
  in flight so FileList cleanup cannot purge source/PDF assets out from under
  background work; terminal cleanup purges deferred sessions.
- Path basename handling uses `to_string_lossy()` for non-UTF-8 path
  components.

### Fixed

- WorkRuntime operation-scoped jobs ignore legacy global cancel.
- Work Center initialization cleans up registered listeners if initial
  operation listing fails.
- Terminal remote-source cleanup is race-guarded, includes merge
  `sourceInputIds`, and purges deferred sessions if background submission
  fails after retain.
- AGENTS.md file lists and crate test commands aligned with current repo
  state.

## [1.0.34] - 2026-06-06

### Added

- Added Audible library pagination up to the API page limit so larger accounts
  can load beyond the previous first 100 titles.
- Added Audible title availability labels, unavailable-title filtering, selected
  count copy, and a Supplemental PDF-only filter in the acquisition dialog.

### Changed

- Requested Supplemental PDFs are now treated as required companion artifacts
  once selected; processing no longer reports success unless the audiobook and
  requested Supplemental PDF both commit.
- Audible acquisition internals now split license selection, audio download,
  materialization handoff, diagnostics, and Supplemental PDF handling into
  clearer backend-owned boundaries.

### Fixed

- Audible catalog/subscription-visible titles that are not downloadable are now
  shown as unavailable instead of allowing acquisition to fail later with a
  missing license URL.
- Supplemental PDF identity checks now reject oversize, symlink, and stale-file
  cases before final output copy.

## [1.0.33] - 2026-06-04

### Added

- Added bundled AAXClean materialization for Audible AAX/AAXC remote
  acquisition on Apple Silicon, with helper protocol and provider secrets kept
  behind the backend runtime.
- Added authenticated Audible Supplemental PDF acquisition and final PDF
  copy-through next to matching processed M4B outputs.

### Changed

- Remote acquisitions now use title-derived materialized audio and Supplemental
  PDF display names, with compact PDF indicators in FileList and inspector
  surfaces.
- Local dev and release build routes now publish and package the AAXClean helper
  sidecar without requiring manual helper-path setup.
- The default developer run command is `bun run app:dev:log`, which refreshes
  `.logs/tauri-dev.log` for shared debugging.

### Fixed

- Provider logout no longer deletes staged materialized files that have already
  been handed to FileList for later processing.
- `app:build:all` now applies the same noninteractive DMG safeguards and bundle
  verification as explicit DMG builds.

## [1.0.32] - 2026-06-03

### Changed

- Tightened Tauri runtime boundary checks so generated command/event values and
  raw Tauri invoke calls stay behind the owned `tauriClient` boundary.
- Replaced the shell version bump helper with a structured Bun script that
  validates and updates all release version surfaces.
- Shrunk repo glossary guidance to implementation-changing terms for clearer
  human and agent routing.

### Fixed

- Removed global remote-source Tauri mocks from frontend test setup so
  remote-source tests must own their provider command fixtures locally.
- Staged binding checks now treat deleted Rust contract files as
  contract-related changes instead of skipping regeneration.

## [1.0.31] - 2026-06-03

### Added

- Added remote source acquisition with an app-level Acquire dialog, Audible
  account handoff, library loading, import-ready M4B handoff, and Supplemental
  PDF copy-through during normal processing.
- Added backend-owned remote source and media contract boundaries for classifying
  provider lanes, protected formats, materialized source files, and staged
  session cleanup.

### Fixed

- Cancelled remote acquisitions now stop the background provider task, keep the
  cancelled terminal state, and remove staged handoff files instead of importing
  late provider results.
- Protected Audible AAX, AAXC, and Dash lanes now stop at the materializer
  boundary instead of entering the FileList as import-ready audio.
- Blocked acquisition handoff paths now purge staged remote files and avoid false
  import success messages.

## [1.0.30] - 2026-05-30

### Changed

- Encoder settings now show effective Auto FDK, Afterburner, sample-rate, and
  channel choices in the existing encoder area with compact resolved-source
  hints.
- File import copy now more clearly describes adding files, folders, or dragged
  folders from one input surface.
- Review proof no longer keeps repo-local `.proof/runs` history by default and
  now runs tracked frontend tests to avoid scratch-file pollution.

### Removed

- Removed the ad hoc FDK FFmpeg override UI, settings, commands, preflight, and
  processing payload plumbing; FDK AAC still works through backend
  auto-detection.

## [1.0.29] - 2026-05-30

### Changed

- Import and processing workflow preparation now use smaller owned helpers,
  reducing duplicated entry and feedback logic in the frontend workflows.
- Repo agent guidance now uses clearer Public API Strip and local trap wording
  for future boundary work.

### Fixed

- File-list selection changes, select-all, clear-selection, and sorting now
  preserve staged metadata drafts or block on validation instead of discarding
  dirty edits.
- Moving or reordering files now refreshes inspector, output, and tag previews
  without resetting dirty metadata fields.
- Metadata Save now shares one entry decision path for busy, processing-active,
  validation, no-op, success, partial failure, and typed failure states.

## [1.0.28] - 2026-05-30

### Changed

- Metadata and runtime settings validation now route through backend-owned
  boundaries with tighter TS/Rust parity.
- Processing terminal outcomes are split into focused classification,
  aggregation, event, and test surfaces.
- External FDK processing internals are split into private Audio Engine modules;
  preview outputs omit chapter markers while full outputs preserve chapters.
- File Import now consumes FileList append/dedupe outcomes, preserving metadata
  drafts and surfacing duplicate-only imports in the import panel.
- Proof routing now uses `bun scripts/proof/runner.ts` with immutable
  `.proof/runs/<run-id>/` summaries, logs, and focused route categories.
- Bun baseline: **1.3.14 stable**. Refresh with `bun upgrade --stable`.

### Removed

- Removed `scripts/proof.sh`; use `bun scripts/proof/runner.ts ...` or the
  existing `bun run proof*` package scripts.

## [1.0.27] - 2026-05-27

App Settings control-plane release.

### Added

- Added durable App Settings so encoder defaults, output defaults, remembered
  output directory, and max-concurrency preference load back into the existing
  controls after app restart.
- Added a backend-owned App Settings module with Tauri commands for reading,
  updating, and resetting persisted preferences.

### Changed

- Existing encoder, output, and concurrency controls now persist accepted
  settings through the App Settings boundary instead of remaining session-only.
- Updated the metadata lookup and cover-art HTTP dependency path to `reqwest`
  0.13.4 after upstream/security review.

### Fixed

- Replaced deprecated Svelte event directives across production components so
  the production build remains free of `event_directive_deprecated` warnings.
- Hardened App Settings persistence against concurrent patch loss, temporary
  settings-file leaks, and max-concurrency drift when persistence fails.

## [1.0.26] - 2026-05-26

Import ingress and file-list workflow release.

### Added

- Added recursive folder import so selecting or dropping a folder loads supported
  audio files from nested directories into the existing flat file list.
- Added macOS Open With support for supported local audio files, including
  cold-start and already-running app imports.
- Added keyboard navigation for the focused file list with Arrow Up/Down,
  Home/End, and Page Up/Down selection movement.

### Changed

- Local audio import now flows through one frontend import boundary backed by
  Rust-owned supported audio metadata and recursive discovery.
- File-list keyboard handling is owned by the focused file list instead of a
  window-level listener for file-specific navigation.
- Replaced the legacy checks script surface with canonical `scripts/proof.sh`
  proof routes for review, package, Rust, media-fixture, frontend, runtime,
  coverage, timing, and dependency validation.

### Fixed

- Large imported file lists now stay inside the input panel and scroll instead
  of forcing the ABB window taller.
- Cleared Svelte/devalue dependency advisories and recorded the preferred
  Bun-first tooling baseline for dependency hygiene work.
- Removed the temporary Node version gate from build, test, lint, and proof
  routes so dependency warning noise does not become repo policy.

### Removed

- Removed the frontend supported-audio allowlist mirror so Rust owns local audio
  importability.

## [1.0.25] - 2026-05-21

Operation lifecycle contract release.

### Added

- Added a processing-owned Backend Lifecycle contract so processing, metadata
  save, and status-panel runtime behavior share explicit operation identity and
  terminal summary vocabulary.
- Added lifecycle operation identity for batch processing, merge processing, and
  metadata save events.

### Changed

- Moved lifecycle event names and progress math out of audio ownership and into
  processing-owned progress/lifecycle surfaces.
- Metadata batch save now reports queue and progress lifecycle truth through
  processing-owned helpers while keeping metadata write policy inside metadata
  APIs.
- Status Panel now consumes backend operation identity from queue/progress events
  without becoming a backend lifecycle owner.

### Fixed

- Tightened Audio Engine boundary documentation and public-strip checks so audio
  no longer exposes lifecycle/status constants.

## [1.0.24] - 2026-05-19

Effect workflow adoption release.

### Added

- Added Effect-backed workflow owners for processing, cancellation, metadata
  save, metadata lookup, output-plan review, toolchain validation, and file
  import so long-running frontend orchestration has typed errors, injectable
  services, and focused test harnesses.
- Added an Effect adoption roadmap artifact to keep the
  architecture direction, milestones, and boundary decisions visible.
- Added local public-strip guidance for the encoder, output, status, and Effect
  workflow surfaces.

### Changed

- Moved process request composition into the status-panel workflow boundary:
  encoder settings now come from the encoder panel, output/naming settings come
  from the output panel, and processing composes both through explicit public
  strips.
- Replaced duplicated workflow Promise/sync harness code with shared AppEffect
  helpers.
- Routed processing directly through the output-plan workflow owner instead of
  the status-panel output-plan review adapter.
- File import, output preflight, metadata save, metadata lookup, cancellation,
  and toolchain validation now use explicit workflow services instead of hidden
  UI orchestration.

### Fixed

- Reviewed no-write batch outputs are now skipped without blocking runnable
  preview or processing jobs.
- Auto and explicit FDK processing now log the resolved processor adapter,
  making External FDK routing auditable during manual testing.
- Removed stale encoder/toolchain/sample-rate mirroring from the output panel so
  encoder changes cannot drift from the processing payload.
- Removed the ineffective import-analysis live-layer dynamic import warning.

## [1.0.23] - 2026-05-14

Resource lifetime and bulk metadata progress release for PR #306.

### Added

- Bulk metadata saves now publish queue-aware progress so metadata batches show
  active, queued, completed, failed, and cancelled jobs in the status panel.
- Status panel queue details can now stay compact by default, with a dedicated
  expandable queue view and summary chips for large metadata batches.

### Changed

- Metadata save batches now run through a backend batch command with
  cancellation checks between files, matching the safer cancellation behavior
  used by audio processing jobs.
- File replacement now uses a shared helper to preserve backup cleanup
  diagnostics and avoid silent cleanup residue.

### Fixed

- Reduced file-handle contention during metadata remuxing and output artifact
  commit replacement by tightening resource lifetimes before source-file
  replacement.
- Backup cleanup leftovers are now surfaced as warnings instead of being hidden
  after an otherwise successful write.

## [1.0.22] - 2026-05-12

Metadata/audio file-handle hygiene release for PR #305 and issue #296.

### Changed

- Metadata reads now classify container routes from an existing FFmpeg input
  context, avoiding probe-only reopen churn before the real metadata read.
- Audio file-list validation now scopes its duration probe before decoder
  inspection opens the same file.

### Fixed

- MP4-family metadata reads now release the FFmpeg probe context before
  `mp4ameta` opens the file, avoiding simultaneous library readers on stricter
  file-locking platforms.
- FFmpeg metadata remux now releases input/output contexts before replacing the
  source path, avoiding same-path replacement while FFmpeg handles are live.

## [1.0.21] - 2026-05-11

Grey-box architecture promotion. This release preserves user-facing behavior
while moving the branch's validated module ownership model into mainline, with
owner manual testing reported clear before promotion.

### Added

- Grey-box review artifacts retained under `docs/artifacts/`, with remaining
  follow-up audits deferred to GitHub issue #304.
- Public API strip contract tests and source allowlisting for the Tauri Runtime
  Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Intent
  Plan, and Status Panel Runtime.
- Nested `AGENTS.md` guidance for the grey-box Public API strips, private
  clusters, allowed edits, and breaking-change triggers.

### Changed

- Reshaped processing planning, output artifact commit, metadata intent
  projection, Tauri runtime adapters, and status-panel runtime behavior behind
  smaller Public API strips.
- Decoupled output artifact commit from `ProcessingContext` by passing an
  explicit `OutputCommitRequest` and cancellation check into the commit
  boundary.
- Refreshed repo guidance, skill docs, and the audio-processing system map for
  the promoted grey-box ownership model.
- Moved the status-panel cover-art tracker test into the local `__tests__`
  cluster.

### Fixed

- Strengthened boundary assertions so generated invokers, metadata intent
  projection, status-panel internals, output artifact commit behavior, and raw
  Tauri invoke usage stay behind their owning boundaries.

### Removed

- Removed the promoted grey-box implementation spec after review and cleaned up
  obsolete root-level scratch probe files.

## [1.0.20] - 2026-05-10

Audio pipeline fallback cleanup, staging hardening, retired perf-infra removal,
and supporting documentation. Prepares `main` for merge by documenting
encoder/staging/finalize ownership without restoring removed compatibility
shims.

### Added

- `docs/artifacts/audio-processing-system-map.html` interactive system-map artifact for
  backend processing seams and boundaries.
- `docs/artifacts/glue-elements-validation-report.md` boundary-orientation notes for
  IPC and orchestration seams.
- `src-tauri/src/audio/AGENTS.md` subsystem guidance aligned with processor staging
  behavior.

### Changed

- Encoder common/context/engine/prepare paths and finalize behavior updated after
  removing audio fallback shims; staging responsibilities expanded and integrated with
  processor orchestration.
- Audio buffer behavior and unit coverage updated to match the non-fallback pipeline.
- Consolidated Codex automation guardrails and refreshed root agent guidance touched by this
  release train.

### Fixed

- Follow-ups from audio staging review, including native AAC regression integration
  coverage and staging edge cases.

### Removed

- Audio encoding/container fallback shims and related constants; fallback registry and
  docs references aligned with root policy.
- Removed the retired local performance benchmark infrastructure, including
  package entrypoints, the manual GitHub Actions workflow, committed benchmark
  scripts/results, and the app E2E perf harness.

## [1.0.19] - 2026-05-01

Metadata boundary consolidation release. This tag makes metadata routing
container-aware, removes stale fallback and movement-tag compatibility claims,
and puts MP4-family and FFmpeg metadata writes behind a single metadata-owned
boundary.

### Changed

- Metadata read/write routing now uses probed container truth instead of
  filename suffixes, so renamed files are handled according to their actual
  container.
- Metadata save, cover-art write, and finalized metadata paths now delegate
  through metadata-owned entry points instead of choosing mp4ameta or FFmpeg in
  command/processor callers.
- Non-MP4 metadata remuxing now maps FFmpeg demuxer names to writable muxers,
  including raw AAC `aac` input to `adts` output.
- Repo metadata guidance and tag-preview copy now describe the current
  canonical/freeform series-tag strategy.

### Fixed

- Raw AAC metadata remux no longer fails by passing FFmpeg's `aac` demuxer name
  as an output muxer.
- Mislabeled MP3-as-M4B files now stay on the FFmpeg metadata path instead of
  entering MP4 atom handling because of the `.m4b` suffix.

### Removed

- Removed the false `ffmpeg_bridge.rs` facade and moved exported helpers to the
  modules that own the work.
- Removed fallback register entries for the old extension-first metadata route
  and Apple movement-tag support.
- Removed Apple movement-tag series read/write support; possible reintroduction
  is deferred pending manual player testing.

## [1.0.18] - 2026-04-30

Terminal-truth, frontend-stack, dependency-tooling, and agent-surface cleanup
release. This tag makes processing results more truthful, completes the
Tailwind Vite plugin cutover, removes stale frontend/documentation residue, and
keeps repo guidance focused on current operating rules instead of past-state
notes.

### Added

- Added a repo-local dependency maintenance skill and snapshot helper for Bun,
  JS package, Rust toolchain, Cargo, workflow-pin, and supply-chain audits.
- Added a decision-alignment skill for substantial planning work now that
  long-lived implementation specs are no longer the default planning surface.

### Changed

- Processing terminal outcome normalization now preserves backend truth for
  success, skipped, cancelled, failed, and mixed batch results.
- Tailwind now runs through the official `@tailwindcss/vite` plugin instead of
  ABB's former PostCSS adapter configuration.
- Output plan panel code now uses value/action helpers owned by the Svelte
  island rather than exported event-shaped handler seams.
- Dependency tooling was refreshed, including Rust toolchain and Cargo lockfile
  updates plus workflow-pin maintenance.
- Repo guidance and glossary surfaces now describe current decision, boundary,
  and agent-operating rules without using completed work as history lessons.

### Fixed

- Release automation now publishes release assets before the immutable release
  step.
- Changed-file harness verification is stable after the UI harness cleanup.
- Processing run results no longer lose failed or cancelled terminal truth in
  batch result normalization.

### Removed

- Removed the Tailwind PostCSS adapter config, dead Tailwind JS config, and
  stale frontend-stack implementation spec.
- Removed legacy UI harness residue and stale output-panel `handlers`/`dom`
  naming that no longer matched the owning code.
- Removed the old planning contract file in favor of the decision-alignment
  skill and current repo guidance.
- Removed a stale history/delta system-shape diagram from docs.

## [1.0.17] - 2026-04-27

Output-truth and boundary cleanup release. This tag makes required output
metadata writes fail truthfully, keeps best-effort passthrough behavior
explicit, and removes stale wrapper/export glue that was obscuring runtime
ownership.

### Changed

- Required metadata, chapter, and explicit cover-art writes now fail the
  affected processing job instead of logging and continuing with a misleading
  success result.
- Passthrough chapter and source-cover preservation remain best-effort, with
  warning diagnostics instead of fatal job results.
- Metadata staging failures now abort processing, selection transitions,
  clear-selection, save, and file-import flows instead of allowing later
  actions to proceed from invalid pending metadata.
- Boundary cleanup removed unused public exports, identity adapters, test-only
  passthroughs, one-line re-exports, and status-panel error helper wrappers.

### Fixed

- Explicit cover clear now suppresses source cover-art passthrough so cleared
  outputs do not silently regain the original embedded cover.
- Single-selection series-part validation now runs before processing starts.
- `rustls-webpki` is patched to `0.103.13`.

## [1.0.16] - 2026-04-25

Formal runtime architecture train closeout release. This tag includes the
already-recorded 1.0.13 through 1.0.15 architecture work plus the final
status-panel state-machine pass and repo hygiene needed to close the train.

### Added

- A pure status-panel state machine for queue, progress, cancellation, skipped,
  failed, and completion-hold transitions.
- A repo-local `audit-boundary-glue` skill for classifying accidental
  indirection, trivial wrappers, dead public exports, identity adapters, and
  related boundary-glue cleanup.

### Changed

- `StatusPanelRuntime` now acts as the side-effect shell for subscriptions,
  timers, feedback, cover art, cancel commands, and rendering while the domain
  state machine owns event truth.
- Issue tracking was narrowed after the runtime architecture train: native AAC
  decoder-probe follow-up is merged into the fixture validation lane, Processing
  Run follow-up now focuses on terminal/artifact truth, and boundary-glue
  cleanup tracks only remaining live wrapper debt.

### Fixed

- Skipped single-job completion now reports as skipped instead of cancelled.
- Command-result reconciliation now repairs missed failed terminal rows as well
  as skipped and cancelled rows.

### Removed

- Removed the completed runtime architecture PR train spec after all five PRs
  landed and related issue state was reconciled.

## [1.0.15] - 2026-04-25

Runtime architecture release that lands the Processing Run boundary and the
Metadata Draft / Intent workflow from the architecture train.

### Added

- A `ProcessingRun` command-layer boundary for preflight and execution flow,
  keeping output planning, job lifecycle, execution routing, cancellation
  interpretation, terminal result handling, and artifact truth behind one
  runtime owner.
- A frontend metadata draft owner that compiles single-selection edits,
  multi-selection staging, metadata lookup application, processing overlays,
  and pending-save state into the existing metadata intent contract.

### Changed

- Processing command orchestration now delegates run construction and terminal
  result normalization through the Processing Run boundary instead of spreading
  those decisions across private command helpers.
- Metadata docs and generated bindings now describe the writable intent surface
  directly: UI draft fields are separate from read-compatible fields, and
  `album_sort` remains explicit backend intent.

### Fixed

- Metadata lookup, save, and processing-overlay paths now preserve set, clear,
  and noop intent consistently through the draft boundary.
- Removed PR4-owned boundary glue around metadata-save state and deduplicated
  series/subseries sequence validation while keeping the public validator names.

## [1.0.14] - 2026-04-24

Audio import hardening release for extension-case handling.

### Fixed

- Audio files with uppercase or mixed-case supported extensions, including
  `.M4B`, now validate through one canonical backend extension table instead of
  failing the file-list format pass.
- Drag-and-drop import coverage now proves uppercase and mixed-case supported
  audio paths are forwarded for analysis.

## [1.0.13] - 2026-04-24

Runtime architecture release that lands the first two PRs of the architecture
train: output planning ownership and external FDK processor ownership, with
real xHE-AAC fixture evidence for the external path.

### Added

- A processor adapter boundary that routes native `ffmpeg-next` processing and
  external FFmpeg/libfdk_aac processing through one processor-owned decision
  point.
- Runtime architecture train notes that fold in the high-ROI boundary-glue audit
  without creating extra GitHub issue churn.

### Changed

- Output naming, collision detection, preview suffix planning, duplicate
  handling, rename candidates, source-overlap blocking, and preflight signatures
  now live behind a deeper output planning boundary.
- External FDK processing moved under `audio::processor` ownership instead of
  being selected directly by command orchestration.
- README, API map, processor docs, and tests now describe ABB as having a native
  in-process path plus an external FFmpeg/FDK adapter path, not an embedded-only
  single-engine runtime.

### Fixed

- Cached output collision planning avoids repeated directory scans and canonical
  source-path work during batch review.
- External FDK stderr-reader tasks are now awaited on early progress/cancel
  errors, child wait errors, and normal process exit.
- Real USAC / xHE-AAC preview validation confirmed both tested sources select
  `aac_at`, force that decoder through the external FDK adapter, and produce
  valid 60-second preview outputs.

## [1.0.12] - 2026-04-22

Status-panel follow-through release that fixes the remaining user-visible
completion-message and progress-stage seams uncovered by post-cutover audit,
while keeping the cutover architecture intact.

### Added

- Regression coverage for batch completion message retention after idle reset.
- Regression coverage proving stage transitions bypass the progress throttle and
  render immediately.

### Changed

- Batch, single, and merge-skip completion holds now use named constants
  instead of scattered literals in the status-panel runtime.

### Fixed

- Batch completion now preserves the terminal user-facing message instead of
  clobbering it with the idle step text in the same tick.
- Mid-job stage transitions are no longer dropped by the one-second progress
  throttle window.
- Removed the dead `clearStatusPanelTransientStatusLock` public status-panel
  export and corresponding stale test mock.

### Removed

## [1.0.11] - 2026-04-21

Repo-hygiene and perf-signal cleanup release that removes the last standing
Biome warning noise from current `main`, splits the overloaded metadata perf
lane into honest synthetic vs network-probe semantics, and defers the broader
settings-panel and metadata-routing work into durable issue owners.

### Added

- A dedicated `metadata-lookup-network-probe` perf benchmark for optional
  external metadata-provider latency measurement in real mode.
- Follow-on issue `#269` to design a basic user-accessible settings panel and
  move durable preferences out of scattered frontend persistence.
- Follow-on issue `#270` to replace suffix-based metadata routing with shared
  container classification and retire `FB-001`.

### Changed

- `metadata-lookup-latency` is now a synthetic-only benchmark for deterministic
  local parsing/mapping work instead of overloading real-mode runs with hidden
  synthetic fallback behavior.
- Perf benchmark selection now respects supported modes so `--all` and
  `--bench-scope core3` do not fabricate inapplicable metadata results.
- Shared checkbox and art-thumbnail CSS rules now follow intentional base-first,
  override-later cascade order so Biome’s selector-specificity lint stays
  meaningful.

### Fixed

- Removed the `FB-016` path where real metadata perf runs silently recorded
  synthetic numbers as successful real-mode results.
- Eliminated the remaining Biome warning noise on current `main`, including the
  repeated manual null-guard patterns Biome was flagging for optional chaining.

## [1.0.10] - 2026-04-20

Focused contract-truth follow-through release that closes the last small
enforcement and invariant gaps left after the `1.0.9` branch cluster.

### Changed

- Encoder construction now treats unresolved `Auto` as an invariant violation
  instead of quietly collapsing it to Native AAC behavior.
- Exported encoder bitrate docs now match the actual validated whitelist,
  including `104` and `120` kbps, across Rust source and generated bindings.
- Fallback policy docs now state that marker-side sunsets are validated as real
  calendar dates, not just register rows.

### Fixed

- `scripts/check-fallback-policy.sh` now rejects malformed source-adjacent
  `sunset=` metadata instead of only checking that the marker field exists.

## [1.0.9] - 2026-04-20

Focused contract-truth release that closes the remaining metadata proof gaps,
hardens fallback sunset enforcement, and reduces handwritten encoder boundary
drift at the Tauri edge.

### Changed

- Preview and full metadata proof now explicitly cover source-derived cover art
  preservation, multi-input chapter behavior, external-FDK preview parity, and
  ffmpeg-path `track` / `disk` reads.
- TypeScript encoder/process boundary types now derive from generated Tauri
  bindings more directly, keeping UI-only wrappers explicit while making Rust
  contract drift fail loudly.

### Fixed

- Fallback policy expiry and renewal checks now validate real calendar dates
  instead of accepting impossible `YYYY-MM-DD` values.
- Preview metadata behavior is now proven against both native and external
  processing paths, including exact cover-art byte preservation and truthful
  preview chapter suppression.
- FFmpeg-only and forced-fallback metadata reads now hydrate `track` and `disk`
  truthfully instead of leaving that proof limited to the mp4ameta path.

## [1.0.8] - 2026-04-20

Release follow-through for the durable output-contract branch, plus the final
status-panel and collision-dialog refinements needed to ship it cleanly.

### Added

- Existing-file collision preflight and one batch-wide resolution flow with
  explicit overwrite, keep-existing, and skip-existing choices before work
  starts.

### Changed

- Decoder/toolchain routing now uses stable decoder IDs at runtime while
  keeping friendly labels as display-only UI text.
- Preview and final outputs now share one backend-owned output plan and
  finalization contract, including `.preview.m4b` path resolution.
- Collision dialog copy and layout were tightened for clearer existing-file
  review without changing policy behavior.
- `track` and `disk` remain readable for compatibility but are no longer part
  of ABB's supported writable metadata contract.

### Fixed

- Existing output files are no longer replaced unless the user explicitly
  selects that policy, and finalization now re-checks destination occupancy
  before commit.
- Skipped batch rows now emit truthful terminal progress, all-skipped runs no
  longer surface as success, and skipped rows are preserved if cancellation
  arrives later.
- Hidden env-driven preview fallback was removed so normal processing no longer
  drifts into preview behavior without an explicit request.
- Frontend progress/status types now include the `skipped` terminal wire stage,
  keeping the TypeScript build aligned with regenerated Rust bindings.

## [1.0.7] - 2026-04-19

Focused contract-hardening release that makes output collision handling explicit
and finalization-aware, stabilizes external decoder routing, and closes the
in-process tail-loss gap in ABB's native pipeline.

### Added

- Backend output preflight planning plus a batch collision-resolution dialog
  with explicit replace, rename, and skip policies before processing starts.
- Skipped-result reporting for collision-dialog behavior and preview-path parity.

### Changed

- Output planning now flows through one backend-owned resolver across preview,
  queueing, execution, and finalization, including `.preview.m4b` artifacts.
- External decoder routing now uses stable decoder IDs instead of friendly UI
  labels, with toolchain validation that fails preflight when the chosen ffmpeg
  cannot honor the requested decoder.
- `track` and `disk` remain readable for compatibility but are no longer part
  of ABB's supported writable metadata contract.

### Fixed

- Existing destinations are no longer replaced unless the user selected an
  explicit collision policy, and source/destination overlap now hard-fails in
  backend planning.
- Preview output naming now matches the real preview artifact path, including
  finalization and fallback flows.
- The native in-process pipeline now drains decoder tail audio correctly and
  flushes accumulator tail samples exactly once, covering both loss and
  duplication regressions.

## [1.0.6] - 2026-04-18

Focused audiobook-edge-case fix release that closes a real xHE-AAC decode
handoff bug, restores large embedded cover-art rendering, and brings output
path behavior closer to the actual processing results ABB now emits.

### Changed

- External FDK processing now carries ABB's selected AAC-family input decoder
  into the subprocess ffmpeg command instead of letting that handoff drift.
- Existing output destinations now resolve to the requested path by default,
  while same-batch naming conflicts are still deconflicted separately.

### Fixed

- xHE-AAC / USAC inputs that validated against Apple AAC no longer fall back to
  ffmpeg's default external decode path during Auto(FDK AAC) processing.
- Large embedded cover art from imported sources now renders reliably in the UI
  without overflowing the old byte-to-data-URL conversion path.
- The Star Trek / Libation regression investigated in this thread now produces
  clean full-length output on the corrected external decoder path.

## [1.0.5] - 2026-04-17

Focused import-workflow release that closes the single-file re-add trap in ABB.
`Add audio files` and file-drop now behave coherently for iterative imports, and
tests now prove that behavior instead of masking it.

### Added

- File-management coverage for add-while-populated behavior, plus mock
  dialog/analyzer paths that can distinguish append from replace during tests.

### Changed

- Status and file-list state now stay truthful under queued batches, fresh file
  imports, and ambiguous thumbnail lookups.
- The Tauri client boundary now uses typed command/event adapters instead of the
  old duplicate metadata-save alias and broader cast-heavy paths.
- Backend terminal progress cleanup now preserves failure `jobId` context
  without degrading cancellation into failed batch results.
- Tooling/docs cleanup pass: shared release shell helpers, cheaper release
  fixture setup, deduped context lists, and compressed historical
  release notes.

### Fixed

- `Add audio files` and file-drop now append unseen audio files into a populated
  list instead of replacing the current import set on repeated adds.
- Additive imports now preserve pending metadata drafts, keep selection stable
  for pure append operations, and ignore duplicate-only batches with a transient
  status instead of mutating the list.
- **Status-panel terminal regression coverage now includes all terminal
  aggregates**. Failed and cancelled terminal aggregates, not just completed
  ones, are covered against stale `currentFile` / `etaSeconds` leakage, and
  batch terminal lifecycle tests now assert only the highest-priority toast
  fires.

### Removed
 
## [1.0.4] - 2026-04-16

Internal state-shape tightening release that continued the "make impossible
states unrepresentable" pass started in 1.0.3. Three boundary/UI refactors,
no wire-format or user-visible behavior changes.

### Changed

- `unwrapGeneratedResult` now accepts only the canonical specta `Result`
  shape, keeps bare values unchanged, and reduces `UnwrapGeneratedResult<T>`
  to three branches (`src/lib/tauri/appError.ts`, `src/lib/tauri/client.ts`).
- Cover art inline status is now a `CoverArtMessage` union instead of the
  `messageText` / `messageVariant` / `messageVisible` triple, and
  `showCoverArtMessage` / `clearCoverArtMessage` are exported for lifecycle
  tests (`src/ui/coverArt/state.svelte.ts`, `src/ui/coverArt/CoverArtIsland.svelte`).
- `ProcessingStatus` is now a `stage`-keyed discriminated union where only
  active stages carry `currentFile` / `etaSeconds`, and the factory and call
  sites route through that shape (`src/ui/statusPanel/state.ts`,
  `src/ui/statusPanel/controller.ts`).

### Added

- New unit coverage for the three seams above:
  - `src/lib/tauri-client.test.ts` pins canonical success/error wrapping,
    bare-value passthrough, and the `status: 'pending'` misclassification guard.
  - `src/ui/coverArt/__tests__/coverArtMessage.test.ts` covers show, auto-dismiss,
    and clear behavior for the inline message timer.
  - `src/ui/statusPanel/__tests__/progressAggregator.test.ts` verifies active
    progress fields do not leak into terminal aggregates.

### Deferred

Intentionally out of scope for this release (tracked for a successor plan):

- `normalizeProcessResult` inner `results` walk / `normalizeAppError(entry.error)`
  pattern in `src/lib/tauri/normalizers.ts`.
- Cleanup of the unused `isProcessingProgressEvent` runtime guard in
  `src/types/events.ts`.

## [1.0.3] - 2026-04-15

Internal seam-tightening release. No user-visible behavior changes; the
`processing-progress` wire format stayed identical, so older frontend builds
remained compatible with this backend.

### Changed

- IPC seam types were tightened end-to-end: Rust `EventStage`, the
  `From<&ProcessingStage>` mappings, regenerated TS bindings, stricter
  normalizers, and a simpler `src/types/events.ts` now agree on the same
  typed wire stage.

### Removed

- Dead TypeScript type definitions: `ProcessingProgress` and `ProcessingStage`
  from `src/types/audio.ts`, plus `MetadataResult`, `WriteMetadataParams`, and
  `WriteCoverArtParams` from `src/types/metadata.ts`.

### DX / Documentation

- Boundary guidance was refreshed in `.agents/skills/job-registry-and-progress/SKILL.md`,
  `src/types/events.ts`, and `src/lib/tauri/normalizers.ts` so the current
  Rust-authoritative wire stage model is documented in one place.

## [1.0.2] - 2026-03-31

Supply-chain install hardening release after the March 31, 2026 axios incident review.

### Changed

- **Dependency Intake Hardening**
  - Enforce lockfile-based Bun installs in CI by switching the perf workflow from `bun install` to `bun ci`.
  - Add a repo-level Bun release-age gate (`bunfig.toml`) so fresh dependency resolutions skip packages published within the last 7 days.

## [1.0.1] - 2026-02-22

Metadata lookup hardening release for legacy-title coverage and no-result recovery UX.

### Added

- **Metadata Lookup**
  - Add OpenLibrary as a selectable metadata source for online lookup.
  - Add a no-results modal CTA (`Use Manual Entry`) that closes lookup and focuses the main metadata title field for faster manual completion on rare/legacy editions.

### Changed

- **Metadata Lookup Resilience**
  - ASIN fast-path now respects selected source intent (OpenLibrary-only lookups skip Audnexus ASIN direct resolution).
  - Merge behavior preserves Audnexus-first ordering while reducing cross-source duplicate title/author matches.
- **DX / Contract Guardrails**
  - Introduce change-aware local IPC binding drift checks (`scripts/check-generated-bindings.sh --mode local`) to reduce iteration churn during non-contract edits.
  - Add binding auto-sync/stage flow for staged Rust IPC contract changes.
  - Add explicit scripts for strict/local/sync binding workflows: `bindings:check`, `bindings:check:local`, `bindings:sync`.

### Fixed

- **Metadata Lookup**
  - Online lookup now returns a real error when all selected providers fail, instead of silently surfacing an empty-result state.
  - OpenLibrary lookups now request `description` in search fields so mapped descriptions can populate when available.

## [1.0.0] - 2026-02-20

Stable architectural milestone: zero-legacy IPC boundary with Svelte app shell.

### Added

- **IPC Boundary**
  - Centralized `tauriClient` (`src/lib/tauri/client.ts`) as canonical command/event interface
  - Generated TypeScript bindings via tauri-specta with drift detection
  - Normalizers for nullish/event-shape handling at boundary (`src/lib/tauri/normalizers.ts`)

- **Svelte Architecture**
  - App shell with reactive state model (`src/App.svelte`, `src/main.ts`)
  - Svelte islands for status panel, file list, cover art, metadata form, job controls, file import, metadata lookup
  - Reactive view-state modules (`.svelte.ts`) for island data binding

- **Guardrails**
  - `scripts/check-no-bridge-imports.sh` — blocks bridge resurrection
  - `scripts/check-no-imperative-dom-runtime.sh` — blocks new imperative DOM in migrated paths
  - `scripts/check-no-legacy-test-contracts.sh` — blocks legacy test contract patterns

- **Testing**
  - `@testing-library/svelte` integration for component testing
  - Colocated test strategy with reactive state assertions

### Changed

- **Architecture**
  - Retired `src/lib/bridge.ts` — all IPC now through `tauriClient`
  - Retired `src/lib/mocks.ts` — test mocking simplified
  - Status panel moved from imperative DOM to reactive view-state ownership
  - File-list control state moved to reactive bindings
  - Metadata save flow uses transient status API
  - Output config processing uses canonical `readOutputConfigForProcessing()`

- **Metadata Intent**
  - Explicit patch semantics (`set|clear|noop`) preserved at boundary
  - Clear intent compiles to Rust sentinels at normalizers, not scattered callsites

### Removed

- `src/lib/bridge.ts` — legacy IPC wrapper
- `src/lib/mocks.ts` — legacy test mocking layer
- `EncoderSettingsProvider` global coupling
- Legacy status-panel DOM fallback renderers
- Imperative row builders from `src/ui/fileList/dom.ts`
- `index.html` UI structure — now in `src/App.svelte`

## [0.1.0] - 2025-12-27

Initial development release with core audiobook processing functionality.

### Added

- **Audio Processing**
  - M4B audiobook creation from MP3, M4A, M4B, and FLAC source files
  - AAC encoding via ffmpeg-next with configurable bitrate and channels
  - Chapter marker preservation from source files
  - Progress tracking with real-time UI updates

- **Metadata Management**
  - Read/write metadata for MP4/M4B files via mp4ameta
  - Support for title, author, narrator, series, and series position
  - Cover art embedding with JPEG/PNG support
  - Audiobookshelf and Apple Books compatible tagging

- **Batch Processing**
  - Process multiple audiobooks in parallel
  - Configurable concurrency limits via job registry
  - Per-job cancellation support
  - Batch metadata application with defaults toggle

- **User Interface**
  - Drag-and-drop file import
  - Metadata editing panel with cover art preview
  - Processing progress display with time estimates
  - Dark theme support

- **Build & Distribution**
  - macOS app bundle generation
  - DMG installer creation script
  - Apple Silicon (ARM64) primary target

### Fixed

- Cover art reads for MP4/M4B files now work correctly
- Channel/sample rate probe failures now fail fast with clear errors
- Decoder flush no longer logs duplicate entries

### Changed

- Migrated from shell FFmpeg to ffmpeg-next Rust bindings
- Refactored audio pipeline to v2 configuration architecture
- Encoder settings now use type-safe configuration objects
