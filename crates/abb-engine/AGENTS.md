# Engine

`abb-engine` decides everything that happens to a user's audiobooks. It has no
UI-toolkit dependency, so the Tauri app, a test, and the `abb-dev` tool drive
the same code.

## Host Interface

- A host builds one `Engine` from `EngineConfig` (`engine.rs` documents each
  field). `Engine::start` clears working files a previous run abandoned (it
  logs and continues on failure), so two engines must not share folders.
- A host calls `Engine` methods and receives `EngineEvent`s; `engine.rs` lists
  the methods. Remote account, auth, library, and acquisition work uses
  session intents and snapshots; its vocabulary is in
  `remote_source/AGENTS.md`.
- The engine runs on the host's tokio runtime: `Engine::start` and every
  async method run inside one. A host with no runtime builds one.
- The working session and the settings each take typed intents and return
  snapshots: `session/AGENTS.md` and `app_settings/AGENTS.md`.
- A new host need (a directory, a platform service, an event) goes into
  `EngineConfig`, `EventSink`, or `Engine`. Engine code never reaches a host
  type.
- Before exiting or reusing the engine's folders, a host awaits
  `Engine::shutdown`. A quit never stops work the user was not asked about:
  `running_work` and `close_for_quit` enforce that (rustdoc in `engine.rs`).
- Every background task the engine starts runs on its one `EngineTasks` owner
  over a `TaskTracker`, so shutdown can wait for it. Short scoped tasks joined
  before their caller returns are the exception. Admission and shutdown share
  one lock: registering visible work and its cancellation handles finishes
  before shutdown enumerates work to stop.
- An accepted intent belongs to the engine. `SessionRun::finish` and settings
  replies only wait; dropping a host wait never drops accepted file work.
- Hosts import intent/snapshot vocabulary from the public modules and call
  `Engine`. Metadata writers, media execution, processing contexts, and
  `WorkRuntime` are crate-internal; the compile-fail examples in `lib.rs` pin
  that.
- `abb-dev` (`src/bin/abb_dev.rs`) is the smallest host. It runs under its own
  identity and state folder; keep it away from the app's settings and
  credentials.

## Owner Routes

- Media execution crosses `crate::audio`; processor adapter selection and
  engine internals stay private to Audio.
- Shared lifecycle vocabulary, internal progress values, active jobs, and
  terminal summaries belong to `crate::processing`. Accepted operation
  identity, snapshots, retention, and operation cancellation belong to
  `crate::work_runtime`.
- The titles being prepared, their metadata drafts, lookup, Save, and
  submission belong to `crate::session`. It writes tags through
  `metadata_save.rs` (one WorkRuntime operation per batch). `cover_source.rs`
  owns the URL and file limits for user-picked covers; `cover_service.rs`
  loads every cover (module doc).
- Online metadata search belongs to `crate::metadata_lookup`. A provider that
  fails while others answer leaves the usable results plus typed diagnostics;
  the search fails only when no selected source can answer
  (`metadata_lookup/service.rs`).
- The settings in effect, their validation, storage, and durability belong to
  `crate::app_settings`; it consults runtime owners for accept/reject rules.
- Remote provider registry, secrets, acquisition, and staged files belong to
  `crate::remote_source`; when an imported download is removed is the
  session's. Provider-private details stay out of other owners, logs, and
  host payloads.
- Metadata reads/writes cross `crate::metadata`. The metadata owner selects
  container handling from actual media classification; callers request an
  outcome rather than choosing MP4/FFmpeg strategy modules.
- Final artifact paths, collision review, replacement, and commit truth cross
  `crate::output_artifact`.
- Exports and previews start from the session (`Submit`, `Preview`). An
  export enters WorkRuntime through `submit_processing_operation`; a preview
  runs `process_inspected_with_options` with a stable identity, actual cancel
  flags, and the shared private snapshot reducer.

## Diagnostics

- `diagnostics.rs` owns the shared stage record and file snapshot format.
  Records never change an operation result. Artifact IDs correlate paths without
  exposing parent folders; processor handoffs link those IDs to job/session IDs.
- Log encoding/metadata/publication transitions and cleanup at their owners.
  A refusal (Start, an audio edit) and a cover that fails to load log one
  line with its kind at the owner; a cover names a remote origin only.
  Metadata diagnostics describe field actions and cover sizes/formats, not tag
  values or artwork bytes. Keep per-packet tracing at debug level.

## Runtime Constraints

- `power::PowerManager` owns the idle-sleep hold (a macOS power assertion, a
  Linux logind idle inhibitor; without D-Bus it logs and work continues) and
  live opt-out. Active encoding, metadata-save, Audible-acquisition, and
  Indexer-handoff scopes hold an `ActiveWork` guard through their final writes
  and cleanup. Acquire after a job's scheduler wait; opening ABB, browsing, and
  external downloader activity do not acquire guards. The manager has no
  polling or idle OS resource.
- Validate input audio paths where they enter the engine with
  `crate::audio::validate_input_audio_path()`. Paths the engine hands back are
  canonical; compare paths in that spelling.
- `JobRegistry` tracks active jobs.
- Production `unsafe` stays inside the FFmpeg and FAAC wrappers under
  `audio/` and `metadata/`; product orchestration, UI contracts, and path or
  output decisions stay safe. Prefer a safe `ffmpeg-next` API where one
  exists. Each `unsafe` block carries a `// SAFETY:` comment stating the
  invariant; Clippy's `undocumented_unsafe_blocks` requires it.
- Keep Clippy allowances local and justified. Code-shape thresholds live at
  root; lint commands and workspace posture are in `scripts/AGENTS.md` and
  root `Cargo.toml`.

## Proof placement

- Proofs that need private media boundaries live under `src/test_cases`, so
  hosts never see those boundaries. `tests/all_tests.rs` proves only the
  separately compiled `abb-dev` host. Runtime construction helpers that serve
  those proofs compile only under `cfg(test)`.
- Private-cluster tests sit in sibling `*_tests.rs` files, declared from the
  owning module with `#[cfg(test)]` and `#[path = "..._tests.rs"]`. A test
  never needs a wider export or a separate integration-test directory.
