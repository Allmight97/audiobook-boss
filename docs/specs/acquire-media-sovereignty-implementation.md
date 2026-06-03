# Acquire + Media Sovereignty — Implementation Handoff

Status: current implementation handoff for the active
`feat/remote-source-acquisition` branch.

Cleanup: delete this spec after implementation, review, validation, docs
alignment, and sync. Distill only durable rules into the owning canon surfaces.

## Outcome

Advance the current branch as a real ABB acquisition feature and begin the ABB
media-sovereignty shape in the same workblock.

The user-visible result:

- The **Acquire** button stays beside **Add Folder**.
- Clicking **Acquire** opens a dedicated app-level acquisition dialog/modal like
  **Find Metadata**.
- The acquisition management UI does not render inside the input/file-order
  panel.
- Audible acquisition reports exact sanitized license/download lane facts needed
  to choose the materializer implementation.
- ABB imports an M4B through the existing file-list analysis path only when the
  provider lane is already import-ready or a later materializer implementation
  produces one.
- The user still reviews metadata, output, and encoder settings before manually
  starting processing.

The architecture result:

- `RemoteSourceRuntime` owns remote-source acquisition.
- AAXClean is an accepted Audible AAX/AAXC materialization candidate under
  `RemoteSourceRuntime`; this workblock does not choose .NET helper versus
  Rust-native implementation.
- ABB owns media facts, plans, capabilities, errors, progress, and provenance in
  a backend-neutral contract layer.
- Audio Engine remains the audio execution owner.
- FFmpeg, `ffmpeg-next`, `ffmpeg-sys-next`, external FFmpeg, AAXClean,
  `mp4ameta`, `rsmpeg`, `rusty_ffmpeg`, or custom FFI details stay private to
  their owning adapters.
- `mp4ameta` remains ABB's M4B/MP4 metadata authority unless implementation
  evidence proves it cannot satisfy ABB's audiobook tag/chapter/artwork needs.

## Current Branch State To Validate

Validate this before editing. The branch already has uncommitted implementation
work.

Known current shape from inspection:

- `src/ui/fileImport/FileImportIsland.svelte` imports the small
  `RemoteSourceAcquireIsland` trigger and renders it beside **Add Folder**.
- `src/ui/remoteSource/RemoteSourceAcquireDialog.svelte` owns the acquisition
  management modal.
- `src/App.svelte` mounts `MetadataLookupIsland` and the remote-source dialog
  at shell level.
- `src/ui/metadataLookup/MetadataLookupIsland.svelte` is the pattern for the
  accepted acquisition UI shape: shell-level modal/dialog, backdrop, header,
  close button, controls, status, results.
- `crates/abb-remote-source-core/src/lib.rs` already models materialized source
  kinds, acquisition stages, AAX/AAXC strategies, and import-ready M4B handoff.
- `src-tauri/src/remote_source/providers/audible/mod.rs` currently rejects
  protected AAX/AAXC before import handoff because Audible materialization is
  not yet implemented.
- `src-tauri/src/audio/processor` already has an internal adapter seam:
  `FfmpegNextProcessor` and external FDK.

Do not treat old plan text as canon. Treat it as prior working context only.
Current repo truth plus this spec drive the implementation plan.

## Before / After

Before:

- Remote acquisition exists as an in-progress branch feature.
- The Acquire button expands remote-source UI inside the input/file-order
  management panel, making an already busy area busier.
- Audible materialization is modeled but not actually able to turn AAX/AAXC into
  an import-compatible M4B.
- Audio processing still reads as `ffmpeg-next`-shaped in places, even though the
  Audio Engine has a private adapter seam.
- Metadata uses `mp4ameta` plus FFmpeg remux helpers.

After:

- Acquire opens an app-level acquisition dialog/modal mounted from `App.svelte`,
  parallel to `MetadataLookupIsland`.
- File import owns local file selection/order. Remote acquisition owns remote
  provider state, library rendering, auth/account state, acquisition progress,
  and diagnostics.
- The Audible provider classifies AAX/AAXC/Dash/Widevine lanes behind a
  provider-private boundary and returns only ABB-owned facts.
- The acquired M4B imports through the existing file-import analysis workflow.
- The Audio Engine remains execution owner, but the public media contract begins
  to use ABB-owned types instead of backend-shaped concepts.
- `mp4ameta` remains the metadata write/readback tool for M4B/MP4 audiobook
  metadata.

## Plain Licensing Rule

AAXClean is allowed as an Audible materialization candidate.

What is allowed:

- Use AAXClean deliberately as a dependency, library, or helper boundary if that
  is the selected implementation path in a later workblock.
- Document the concrete license and binary-distribution implications in the
  implementation notes or PR body.
- Keep AAXClean inside the Audible materializer boundary.

What is not allowed without explicit owner approval:

- Paste AAXClean, Libation, or other third-party source into ABB.
- Closely port third-party source line-by-line or structure-by-structure while
  pretending it is new ABB code.
- Use Libation as a helper app or tell the user to use another tool and import
  manually.
- Let AAXClean/provider/protected-content types leak into TS payloads, processing,
  metadata, output artifact, or general Audio Engine public surfaces.

GPL is not a ban. GPL creates obligations. Do not turn licensing into a fake
reason to avoid the feature. Do record what ABB is distributing and what source
or notices are required.

## Scope

In scope:

- Current branch audit before edits.
- Acquire UI reshaping into a shell-level modal/dialog.
- Remote-source frontend state/actions organized so the trigger button and
  dialog can share state without embedding the dialog in file import.
- Audible materializer boundary preparation under
  `src-tauri/src/remote_source/providers/audible/` or a narrower private module
  inside that provider cluster. AAXClean helper versus Rust-native
  implementation is not selected in this workblock.
- ABB-owned media contract layer, likely `crates/abb-media-core`, if the
  implementation plan proves a crate is the cleanest home.
- Generated Rust-to-TS contract updates required by the implementation.
- Supplemental PDF handling already planned for remote-source acquisition.
- Direct tests/checks for touched surfaces.
- Manual live-account validation by the repo owner.

Out of scope:

- Libation helper dependency.
- Full FFmpeg replacement.
- `rsmpeg`, `rusty_ffmpeg`, or custom FFmpeg FFI migration beyond defining the
  contract shape that would make such a migration possible later.
- Global UI redesign.
- macOS menu integration.
- User settings panel.
- Automatic processing after acquisition.
- Remote providers beyond Audible.
- Proof-runner or test-infrastructure redesign.

## Work Plan

1. Audit current branch state.

   Inspect the dirty worktree, generated bindings, changed tests, and current
   remote-source branch behavior. Do not overwrite in-flight work blindly.

2. Reshape Acquire UI ownership.

   Keep a small trigger button beside **Add Folder**. Move the acquisition
   management surface to a shell-level component mounted near
   `MetadataLookupIsland` in `App.svelte`.

   Accepted shape:

   - app-level modal/dialog
   - title: **Acquire Audiobooks**
   - provider selector, with Audible as the only selectable source for now
   - account/auth state
   - library load/refresh
   - typeahead filtering
   - multi-select title list
   - Supplemental PDF default-on controls only when available
   - acquisition progress and diagnostics
   - close/logout/cancel controls as appropriate

   The input/file-order panel must not own or render the remote library.

3. Add ABB-owned media contract.

   Add the smallest coherent pure media contract needed by this PR. Prefer a
   first-party `abb-media-core` crate if it improves clarity without adding
   empty ceremony.

   Candidate contract concepts:

   - `MediaInputId`
   - `MaterializedMediaFile`
   - `MediaContainerKind`
   - `MediaSourceKind`
   - `MediaFileFacts`
   - `MediaStreamFacts`
   - `ChapterSet`
   - `ArtworkRef`
   - `MediaCapabilityProfile`
   - `MediaProgress`
   - `MediaErrorKind`
   - `MaterializationManifest`

   These types must not depend on FFmpeg, AAXClean, provider payloads,
   `mp4ameta`, Tauri, keyring, or generated TS glue.

4. Prepare the Audible materializer boundary.

   Keep materializer-facing code provider-private. This workblock classifies
   provider-owned downloaded/protected source material plus provider-owned
   decryption/session facts, then reports the materializer lane without choosing
   AAXClean helper versus Rust-native implementation.

   The public result remains:

   - `MaterializedSourceFile`
   - `SupplementalAsset`
   - typed diagnostics
   - sanitized progress

   No raw license blobs, provider responses, credentials, tokens, cookies,
   decryption keys, or protected intermediates cross into frontend state or logs.

5. Preserve normal ABB workflow after acquisition.

   Once a title materializes, including future materializer work:

   - validate the M4B as an audio input
   - import it through the existing file-list analysis path
   - attach Supplemental Assets by `inputId`
   - leave processing manual
   - copy Supplemental PDFs only after the matching final M4B succeeds
   - purge acquired session artifacts according to existing session cleanup rules

6. Keep implementation notes.

   Maintain a temporary implementation notes file while working. Record:

   - AAXClean/helper-versus-Rust-native evidence collected
   - license/distribution note
   - provider behavior observed
   - decisions not obvious from this spec
   - validation performed
   - any user-visible behavior changes

   Delete the notes or collapse durable facts into the right canon surface before
   merge.

## Stop And Ask Before Changing Direction

Ask before shipping any of these:

- provider password fields in Svelte
- embedded provider login in a privileged Tauri webview
- raw provider/protected-content payloads in generated TS types
- plaintext credential fallback
- Libation helper usage
- automatic processing after acquisition
- replacing `mp4ameta`
- migrating the general Audio Engine from `ffmpeg-next` to another backend
- a true OS/Tauri child window instead of an in-app modal/dialog
- broad UI redesign beyond the acquisition dialog

Do not use "blocker" language. State the concrete choice and the consequence.

## Verification

Use direct commands for the touched surfaces. Do not add a custom proof runner.

Expected direct checks before PR, adjusted by actual touched files:

- `cargo fmt --all -- --check`
- `bun run fmt:check`
- `bun run lint:check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `bash scripts/check-generated-bindings.sh --mode local`
- `bash scripts/check-public-api-strips.sh`
- `bash scripts/check-no-bridge-imports.sh`
- `bash scripts/check-fallback-policy.sh`
- `cargo nextest run -p abb-remote-source-core`
- new core crate tests if `abb-media-core` is added
- focused `cargo nextest` tests for `remote_source`
- focused `bun run test -- ...` for remote-source UI, tauri client contracts,
  session assets, behavior contracts, and modal/dialog behavior
- `bun run build`

Manual owner validation:

- Open ABB and click **Acquire**.
- Confirm the acquisition UI opens as a dedicated app-level dialog/modal, not
  inside the input/file-order panel.
- Authenticate or refresh Audible account state.
- Load Audible library.
- Filter and select one or more titles.
- Include a Supplemental PDF when available.
- Acquire at least one title.
- Confirm the visible status and sanitized logs name the license/download lane.
- If ABB receives an import-ready M4B lane, confirm it imports into ABB's file
  list and normal processing can start manually.
- Logout/purge and confirm acquired session artifacts are gone.
- Confirm no secrets appear in visible logs/events/errors.

## Handoff Prompt For Implementation Agent

You are on the active `feat/remote-source-acquisition` branch. Validate the
current dirty worktree before editing.

Implement [docs/specs/acquire-media-sovereignty-implementation.md](/Users/jstar/Projects/audiobook-boss/docs/specs/acquire-media-sovereignty-implementation.md).

Create a short implementation plan from the current repo state, then execute it.
The key outcomes are:

- Acquire opens an app-level acquisition dialog/modal like metadata lookup.
- Remote acquisition UI is no longer embedded in the file-import/file-order
  panel.
- Audible license/download facts are unblocked so the next materializer choice
  can be made from observed provider lanes.
- ABB gains the first coherent media-contract layer so media facts/plans/errors
  are ABB-owned and backend-neutral.
- Audio Engine remains execution owner.
- `mp4ameta` remains metadata authority.
- Normal ABB processing remains manual after acquisition.

Use plain implementation language. Do not treat GPL as a ban. Do distinguish
using AAXClean as a dependency/helper from copying or closely porting third-party
source. Document concrete license/distribution implications for the chosen
materializer path.

Keep temporary implementation notes while working and remove or collapse them
before merge.
