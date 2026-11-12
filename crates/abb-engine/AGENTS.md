# Engine

- `Engine::start` is synchronous and runs with no Tokio runtime; async methods
  need the host's runtime. `open_audio_files` is async only so its caller
  supplies one (the macOS opened-files thread in `src-tauri`).
- A new host need (a directory, a platform service, an event) goes into
  `EngineConfig`, `EventSink`, or `Engine`; engine code never reaches a host
  type.
- Before exiting or reusing the engine's folders, a host awaits
  `Engine::shutdown`. A quit never stops work the user was not asked about.
- Task admission and shutdown share one lock, so registering visible work and
  its cancellation handles finishes before shutdown enumerates work to stop.
- `abb-dev` runs under its own identity and state folder; keep it away from
  the app's settings and credentials. A real Audible download through
  `abb-dev audible` is the owner's call (verify-abb skill).
- Provider-private details stay out of other owners, logs, and host payloads.
- Callers request a metadata outcome; the metadata owner picks MP4 or FFmpeg
  handling from the actual media, never the caller.
- Log transitions and cleanup at their owners. A refusal or a failed cover
  load logs one line with its kind; a cover names a remote origin only.
  Metadata diagnostics describe field actions and cover sizes and formats,
  never tag values or artwork bytes. Per-packet tracing stays at debug.
- `power::PowerManager` holds idle sleep off through a macOS power assertion
  or a Linux logind inhibitor; without D-Bus it logs and work continues.
  Encoding, metadata saves, Audible acquisition, and Indexer handoff hold an
  `ActiveWork` guard after their scheduler wait and through final writes and
  cleanup; opening ABB and browsing do not.
- A lock that guards state a panic cannot half-apply is taken with
  `PoisonError::into_inner`, so one panic does not fail every later call
  (session transitions, job-registry admission, `WorkRuntime::operations`).
- Validate input audio paths where they enter the engine
  (`validate_input_audio_path`). Paths the engine hands back are canonical;
  compare paths in that spelling.
- Proofs that need private media boundaries live under `src/test_cases`, so
  hosts never see those boundaries; `tests/all_tests.rs` proves only the
  separately compiled `abb-dev`. A test never needs a wider export.
