# Changelog

All notable changes to AudioBook Boss™ will be documented in this file.

## [Unreleased]

### Added

### Changed

### Fixed

### Removed

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
- Consolidated Codex hook guardrails and refreshed root agent guidance touched by this
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
- An ignored `ABB_XHE_AAC_FIXTURE` regression lane for proving real USAC /
  xHE-AAC fixtures through the external FDK preview path.
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
  fixture setup, deduped context-surface lists, and compressed historical
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
  - Add hook-based binding auto-sync/stage flow in `.githooks/pre-commit` for staged Rust IPC contract changes.
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
