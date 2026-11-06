# Tauri Host

`src-tauri` is one UI host for the engine. It carries requests from the
webview to `abb_engine::Engine`, forwards `EngineEvent`s as Tauri events, and
owns the window, native dialogs, and quit handling. Product rules live in the
engine (`crates/abb-engine/AGENTS.md`). When the host needs something the
engine does not offer, add it to the engine's host interface.

## What the host owns

- **Contract.** `src/ipc_contract.rs` registers every command and event;
  generated TypeScript bindings follow it.
- **Intent ordering.** The webview can deliver requests out of order. The
  frontend numbers each session and settings intent, and `intent_order.rs` runs
  each one after all earlier ones, except that a number still missing after
  `MISSING_INTENT_WAIT` is skipped with a warning. An intent from a replaced
  frontend is refused. An intent that arrives after a later one has run is
  refused, never applied out of order. The check and the claim happen in one
  step, so a reload between them cannot hand an old frontend's intent a turn.
- **Log file.** `log_file.rs` sends the log to stderr and to `abb.log` in the
  app log folder (macOS `~/Library/Logs/com.audiobook-boss`; Linux
  `$XDG_DATA_HOME/com.audiobook-boss/logs`). Why: a Finder-launched app has no
  readable stderr. A launch appends, so a second copy never removes a running copy's
  log; only a launch that finds `abb.log` over 5 MB moves it to
  `abb.previous.log`. If the file cannot open, the app logs to stderr and warns
  once.
- **Second copy.** When `Engine::start` fails with `AlreadyRunning`, setup
  hides the window, shows the message in a native dialog, and exits on OK
  without an engine. `Quit` and `FrontendLink` are managed before the engine
  starts so that exit path and the hidden webview's commands never panic.
- **Quit.** `lib.rs` holds every quit until `Engine::shutdown` settles. Running
  work, a slow shutdown, and unsaved settings reach the user as choices. The
  process exits before the engine settles only when the user chooses Quit Now,
  and with unsaved settings only when the user chooses Quit Anyway. Saves
  waiting only behind a review or preview are written during shutdown, and the
  quit prompt says so first.

## Commands

- A command is thin: take the IPC inputs, call one `Engine` method, and return
  `CommandResult<T>`. Errors reach the frontend as `AppErrorEnvelope`.
- Session and settings changes go through `session_dispatch` and
  `settings_dispatch` in `commands/session.rs`, which take a turn from the
  intent order first. A new user action is a new intent variant in the engine,
  not a new command. Remote source account, authentication, and library
  actions are session intents.
- A separate command serves a read outside a snapshot, or an owner that takes
  no intents yet. Covers use the `abb-cover` scheme.
- `open_preview_output` is the one session action with its own command: it
  takes a session turn, claims the preview through `TakePreviewOutput`, and
  opens the path with `OpenerExt::open_path`. Why: opening needs a host
  capability, and the webview must never name a path to open. An open
  failure returns the reply with a `Rejected` outcome so its update still
  applies.
- The engine validates paths, metadata intent, output artifact truth, and
  operation lifecycle. A command passes the request through.
- Register a new command in `ipc_contract.rs`, then run
  `bun run bindings:generate` (the host lane checks drift). Run it after an
  engine behavior change too: `src/lib/generated/fixtures/` records real engine
  outcomes and the changed session snapshot parts.

## Tests

A test of product behavior, including what a command reaches, belongs with the
engine owner that decides it.

`commands/session_invoke_tests.rs` sends commands through Tauri's mock runtime
and the real invoke handler (`ipc_contract::builder::<MockRuntime>()`, so
`open_preview_output` is generic over the runtime). It pins the camelCase
argument and reply names the webview sees, the error envelope for a refused
intent number, and the managed state a command needs. Send a numbered intent only
after the one before it: a gap stalls the test for `MISSING_INTENT_WAIT`, then
`IntentOrder` skips the number and refuses it as late.

`src/bin/export_bindings` records the fixtures from the real engine: the intent
sequences are in `scenarios.json`; `"binding": "current"` in a step becomes the
form binding the engine shows before that step. The check runs on macOS and
Linux, so a value that differs between machines is pinned in `pin_machine_values`; a new one
makes the other OS report a stale fixture. Name a title only after importing
it, or the recorder panics.
