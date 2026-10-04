# Command Boundary

Applies to Tauri command modules under `src-tauri/src/commands/`. A command
carries one request to the engine and returns its answer.

## Preferred Path

- Keep a command thin: take the IPC inputs, call one `Engine` method, and
  return `CommandResult<T>`.
- Session and settings changes go through `session_dispatch` and
  `settings_dispatch` in `session.rs`, which take a turn from the intent
  order before calling the engine. A new user action is a new intent variant
  in the engine, not a new command.
- Add a separate command only for a read that is not part of a snapshot (for
  example cover bytes) or for an owner that does not take intents yet.
- Remote source account, authentication, and library actions are session intents;
  they do not have standalone Tauri commands.
- Register command and event changes in `src-tauri/src/ipc_contract.rs` and keep
  generated TypeScript bindings in sync.

## Hard Invariants

- Return `AppError`/`AppErrorEnvelope` through `CommandResult<T>`; do not expose
  ad hoc string error contracts to the frontend.
- The engine validates paths, metadata intent, output artifact truth, and
  operation lifecycle. A command does not repeat, pre-check, or substitute for
  those decisions.
- Do not bypass `JobRegistry` or `WorkRuntime` for long-running operation
  lifecycle, snapshots, or cancellation behavior.

## Done Criteria

- Command additions or shape changes have matching Specta registration and
  binding drift checks.
- The behavior a command reaches is proved by the engine owner's tests.
- Direct review commands from `README.md` and `scripts/AGENTS.md` are the
  default verification path for command, contract, or generated-binding changes.
