# Tauri Host

`src-tauri` is one UI host for the engine. It carries requests from the
webview to `abb_engine::Engine`, forwards `EngineEvent`s as Tauri events, and
owns the window, native dialogs, and quit handling. Product rules live in the
engine (`crates/abb-engine/AGENTS.md`): a decision about audiobooks, settings,
or work lifecycle goes there. When the host needs something the engine does not
offer, add it to the engine's host interface.

## What the host owns

- **Contract.** `src/ipc_contract.rs` registers every command and event;
  generated TypeScript bindings follow it.
- **Intent ordering.** The webview can deliver requests out of order. The
  frontend numbers each session and settings intent, and `intent_order.rs` runs
  each one after all earlier ones. An intent from a replaced frontend is
  refused. An intent that arrives after a later one has run is refused, never
  applied out of order.
- **Event forwarding.** `events.rs` maps each `EngineEvent` to one Tauri event
  and holds no state.
- **Covers.** `cover_protocol.rs` serves the `abb-cover` scheme: it passes each
  request path to `Engine::cover` and maps the answer to an HTTP status.
- **Quit.** `lib.rs` holds every quit until `Engine::shutdown` settles. Running
  work, a slow shutdown, and unsaved settings reach the user as choices. The
  process exits early only when the user chooses Quit Now.

## Commands

Command modules live in `src/commands/`.

- A command is thin: take the IPC inputs, call one `Engine` method, and return
  `CommandResult<T>`. Errors reach the frontend as `AppErrorEnvelope`.
- Session and settings changes go through `session_dispatch` and
  `settings_dispatch` in `commands/session.rs`, which take a turn from the
  intent order first. A new user action is a new intent variant in the engine,
  not a new command. Remote source account, authentication, and library
  actions are session intents.
- A separate command serves a read outside a snapshot, or an owner that takes
  no intents yet. Covers use the `abb-cover` scheme.
- The engine validates paths, metadata intent, output artifact truth, and
  operation lifecycle. A command passes the request through.
- A command change updates `ipc_contract.rs` and the generated bindings. Checks:
  `scripts/AGENTS.md`.

## Tests

Host tests cover what the host owns: intent ordering, window sizing, the quit
prompt and shutdown wait, the `abb-cover` response mapping, the frontend log
command, and the generated binding file's format. The binding checks and the
frontend contract tests prove command and event shapes. A test of product
behavior, including what a command reaches, belongs with the engine owner that
decides it.
