# Engine

`abb-engine` decides everything that happens to a user's audiobooks. It has no
UI-toolkit dependency, so the Tauri app, a test, and the `abb-dev` tool drive
the same code.

## Host Interface

- A host builds one `Engine` with `EngineConfig`: a cache folder, a config
  folder, an identity that scopes stored credentials, and an `EventSink`.
  `Engine::start` clears working files a previous run abandoned, so two
  engines must not share those folders.
- A host calls `Engine` methods and receives `EngineEvent`s; `engine.rs` lists
  the methods. Remote account, auth, library, and acquisition work uses
  session intents and snapshots; its owned vocabulary is in
  `remote_source/AGENTS.md`.
- The engine runs on the host's tokio runtime: `Engine::start` and every
  async method must be called inside one. A host with no runtime builds one.
- The working session and the settings each take typed intents and return
  snapshots: `session/AGENTS.md` and `app_settings/AGENTS.md`.
- A new host need (a directory, a platform service, an event) is added to
  `EngineConfig`, `EventSink`, or `Engine`. Engine code never reaches a host
  type.
- Before exiting or reusing the engine's folders, a host awaits
  `Engine::shutdown`: it refuses new exports and acquisitions, cancels running
  ones, a running preview, and any submission waiting at collision review,
  and waits for every background task and for the Saves, submissions, and
  settings writes already under way, accepted remote disconnects and credential
  writes, and started keychain reads, so saves waiting on them are written.
  Remote registration and network reads stop before credential persistence.
  Metadata Saves are not cancelled. `Engine::running_work` tells a host what
  quitting would stop.
- Every background task the engine starts runs on its one `EngineTasks` owner over a `TaskTracker`
  (`tokio_util`), never a bare `tokio::spawn`, so shutdown can wait for it.
  Short scoped tasks joined before their caller returns are the exception.
  Admission and shutdown share one lock: registering visible work and its
  cancellation handles must finish before shutdown enumerates work to stop.
- An accepted intent belongs to the engine. `SessionRun::finish` and settings
  replies only wait; dropping a host wait never drops accepted file work.
- Hosts import intent/snapshot vocabulary from the public modules and call
  `Engine`; metadata writers, media execution, processing contexts, and
  `WorkRuntime` are crate-internal. Compiler-checked negative API examples
  in `lib.rs` prevent tests from reopening those safety bypasses.
- `abb-dev` (`src/bin/abb_dev.rs`) is the smallest host. It runs under its own
  identity and state folder; keep it from reading the app's settings or
  credentials.

## Owner Routes

- Media execution crosses `crate::audio`; processor adapter selection and
  engine internals stay private to Audio.
- Shared lifecycle vocabulary, internal progress values, active jobs, and terminal
  summaries belong to `crate::processing`. Accepted operation identity,
  snapshots, retention, and operation cancellation belong to
  `crate::work_runtime`.
- The titles being prepared, their metadata drafts, lookup, Save, and
  submission belong to `crate::session`. It writes tags through `metadata_save.rs` (one WorkRuntime
  operation per batch, with crate-internal request/result types) and loads user-picked covers through `cover_source.rs`,
  which owns the URL and file limits.
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
  Metadata diagnostics describe field actions and cover sizes/formats, not tag
  values or artwork bytes. Keep per-packet tracing at debug level.

## Runtime Constraints

- `power::PowerManager` owns the macOS idle-sleep assertion and live opt-out.
  Active encoding, metadata-save, Audible-acquisition, and Indexer-handoff scopes hold an
  `ActiveWork` guard through their final writes and cleanup. Acquire after a
  job's scheduler wait; opening ABB, browsing, and external downloader activity
  do not acquire guards. The manager has no polling or idle OS resource.
- Validate input audio paths where they enter the engine with
  `crate::audio::validate_input_audio_path()`. Paths the engine hands back are
  canonical; compare paths in that spelling.
- Use `JobRegistry` for active-job tracking. Cancellation is per title:
  WorkRuntime's flags for an export, the session's for a preview.
- Run CPU-bound encoding and heavy synchronous work through
  `tokio::task::spawn_blocking` or an equivalent blocking-safe path.
- Keep long-running progress and terminal outcomes observable through the
  owning lifecycle surface.
- Before changing production Rust `unsafe`, read
  `docs/unsafe-code-register.md`; update it if scope, purpose, or blast radius
  changes. Unsafe details stay inside the required FFmpeg/FFI boundary.
- Keep Clippy allowances local and justified. Code-shape thresholds live at
  root; lint commands and workspace posture are in `scripts/AGENTS.md` and
  root `Cargo.toml`.

Use the nearest subsystem guidance for its public interface and traps, and
`scripts/AGENTS.md` for checks matching the changed boundary.

## Proof placement

Real-file engine proofs live under `src/test_cases` so they can use private
media boundaries without exposing them to hosts. `tests/all_tests.rs` proves
only the separately compiled `abb-dev` host. Runtime construction helpers that
serve those proofs compile only under `cfg(test)`.
