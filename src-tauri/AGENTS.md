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
  each one after all earlier ones. An intent from a replaced frontend is
  refused. An intent that arrives after a later one has run is refused, never
  applied out of order.
- **Second copy.** When `Engine::start` fails with `AlreadyRunning`, setup
  hides the window, shows the message in a native dialog, and exits on OK
  without an engine. `Quit` and `FrontendLink` are managed before the engine
  starts so that exit path and the hidden webview's commands never panic.
- **Quit.** `lib.rs` holds every quit until `Engine::shutdown` settles. Running
  work, a slow shutdown, and unsaved settings reach the user as choices. The
  process exits early only when the user chooses Quit Now.

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
  `bun run bindings:generate` (the host lane checks drift).

## Tests

A test of product behavior, including what a command reaches, belongs with the
engine owner that decides it.
