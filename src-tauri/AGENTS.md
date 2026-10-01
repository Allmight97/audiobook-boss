# Tauri Host

`src-tauri` is one UI host for the engine. It carries requests from the
webview to `abb_engine::Engine`, forwards `EngineEvent`s as Tauri events, and
owns the window, native dialogs, and the quit warning. Product rules live in
the engine: `crates/abb-engine/AGENTS.md`.

## What The Host Owns

- **Command and event contract.** `src/ipc_contract.rs` registers every
  command and event; generated TypeScript bindings follow it. Command rules:
  `src/commands/AGENTS.md`.
- **Intent ordering.** The webview can deliver requests out of order, so the
  frontend numbers each session and settings intent and `intent_order.rs`
  runs each one after all earlier ones. `attach_frontend` starts a new
  numbering; intents from a replaced frontend are refused. An intent stops
  waiting for a missing earlier one after `MISSING_INTENT_WAIT`. Session and
  settings intents are numbered separately (`FrontendLink`).
- **Event forwarding.** `events.rs` maps each `EngineEvent` to one Tauri
  event. It adds no state and drops nothing.
- **Quit warning.** `lib.rs` asks before quitting or closing while
  `Engine::waiting_metadata_writes` reports saves that are waiting for an
  export to finish.

## Rules

- A decision about audiobooks, settings, or work lifecycle goes in the engine.
  When the host needs something the engine does not offer, add it to the
  engine's host interface.
- Host tests cover what the host owns: intent ordering, window sizing, the
  frontend log command, and the generated binding file's format. Command and
  event shapes are proved by the binding checks and the frontend contract
  tests. A test of product behavior belongs with the engine owner that
  decides it.

Checks for a changed command, event, or binding: `scripts/AGENTS.md`.
