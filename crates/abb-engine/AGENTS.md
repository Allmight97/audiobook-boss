# Engine

- `Engine::start` is synchronous and runs with no Tokio runtime; async methods
  need the host's runtime. `open_audio_files` is async only so its caller
  supplies one (the macOS opened-files thread in `src-tauri`).
- `Engine::start` holds a lock on its cache folder, because a second engine's
  startup cleanup would delete the first one's live exports and downloads. If
  the lock cannot be taken for any reason, `Engine::start` refuses before
  abandoned-work cleanup runs.
- A new host need (a directory, a platform service, an event) goes into
  `EngineConfig`, `EventSink`, or `Engine`; engine code never reaches a host
  type.
- Before exiting or reusing the engine's folders, a host awaits
  `Engine::shutdown`. A quit never stops work the user was not asked about.
- Task admission and shutdown share one lock, so registering visible work and
  its cancellation handles finishes before shutdown enumerates work to stop.
- `abb-dev` runs under its own identity and state folder; keep it away from
  the app's settings and credentials. A real Audible download
  (`scripts/abb-dev.sh audible`) runs only when the owner asks, for the fewest
  titles, because each spends the owner's account; then delete its state
  folder, because it holds decrypted audiobooks.
- Provider-private details stay out of other owners, logs, and host payloads.
- Log transitions and cleanup at their owners. A refusal or a failed cover
  load logs one line with its kind; a cover names a remote origin only.
  Metadata diagnostics describe field actions and cover sizes and formats,
  never tag values or artwork bytes. Per-packet tracing stays at debug.
- A lock that guards state a panic cannot half-apply is taken with
  `PoisonError::into_inner`, so one panic does not fail every later call
  (session transitions, job-registry admission, `WorkRuntime::operations`).
- Validate input audio paths where they enter the engine
  (`validate_input_audio_path`). Paths the engine hands back are canonical;
  compare paths in that spelling.
- Media proofs live in `abb-media-pipeline`. Engine proofs that need private
  engine boundaries live under `src/test_cases`, so hosts never see those
  boundaries; `tests/all_tests.rs` proves only the separately compiled
  `abb-dev`. A test never needs a wider export.
- Test helpers that use no engine type (WAV and MP3 writers, `ffmpeg` and
  `ffprobe` readback, log capture, polling) live in the dev-only
  `abb-test-fixtures` crate, so every crate's tests share one copy;
  `check-rust-tiers.sh` keeps it out of production dependency trees. Helpers
  that build engine types stay in `src/test_cases`.
- A test that needs the scheduler busy runs a real export held at its
  output-file lock (`test_cases/support/blocked_export.rs`), not a
  test-only registry hook, so it still holds when the registry moves crates.

## Outside facts

- `Cargo.toml` reqwest: each server picks HTTP/2 or HTTP/1.1; HTTP/3 stays
  off, because it needs the `reqwest_unstable` cfg and has no fallback
  (seanmonstar/reqwest#2303).
- `Cargo.toml` keyring: the Linux Secret Service store uses
  `rt-async-io-crypto-rust`, because zbus's tokio feature is crate-wide and
  keepawake's blocking bus calls would then start a runtime inside the
  engine's runtime.
- `cover_source.rs`: an IP-literal host never reaches the resolver, so it is
  judged from the parsed host, not its bracketed text. A resolved address
  carries port 0 because reqwest replaces it with the URL's port.
- `metadata_lookup/providers/openlibrary.rs`: Open Library has no narrator or
  duration data.
- `abb-dev audible`: a terminal in canonical mode drops a line past 1024
  bytes, and the sign-in landing URL is longer, so the read turns canonical
  mode off; a blocked stdin read keeps the runtime from shutting down, so the
  process exits after restoring the terminal.
- `abb-dev parity` reads outputs with the `ffmpeg` and `ffprobe` commands,
  which share no code with `abb-ffmpeg`, so a layer defect cannot hide on
  both sides; it ignores the `encoder` tag, which only names the muxer.
- Tests: root ignores permission bits, so a write-failure test puts a regular
  file at the parent path. Under uid 0, `tests/all_tests.rs` runs the host
  unprivileged from a copy beside the book, because that user cannot
  traverse a private checkout such as one under `/root`.
- `test_cases/integration_session_desk.rs`: a child process another test
  thread spawns inherits the descriptor table and holds the folder's `flock`
  until its exec, about a millisecond.
