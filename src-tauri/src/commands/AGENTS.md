# Command Boundary Directives

## Scope

- Applies to Tauri command modules under `src-tauri/src/commands/`.
- Commands are IPC ingress adapters, not owners of product rules or deep
  processing behavior.

## Preferred Path

- Keep command functions thin: parse IPC inputs, validate boundary data, call
  the owning backend module, and return `CommandResult<T>`.
- Route product behavior through the owning Public API Strip:
  `crate::audio`, `crate::metadata`, `crate::processing`,
  `crate::work_runtime`, `crate::output_artifact`, or `crate::app_settings`.
- Register command and event changes in `src-tauri/src/ipc_contract.rs` and keep
  generated TypeScript bindings in sync.
- Use `tokio::task::spawn_blocking` for synchronous file/media work reached from
  async commands.
- Keep provider-specific lookup code inside its command family and service
  modules; command functions should not grow into HTTP clients or mappers.
- Provider degradation behavior must be explicit in the command response and
  covered by focused tests.

## Hard Invariants

- Validate user-supplied paths at this boundary before domain modules receive
  them: input audio paths through `crate::audio::validate_input_audio_path()`
  (extension allowlist + traversal/canonicalization); execution-time path
  planning is owned by `processing::plan`, and requested/resolved artifact paths,
  collision, and parent-dir creation by `crate::output_artifact`. Map path
  errors to `AppError` without leaking sensitive absolute paths to the UI.
- Return `AppError`/`AppErrorEnvelope` through `CommandResult<T>`; do not expose
  ad hoc string error contracts to the frontend.
- Do not bypass `JobRegistry` or `WorkRuntime` for long-running operation
  lifecycle, snapshots, or cancellation behavior.
- Do not encode metadata intent, output artifact truth, processing lifecycle
  vocabulary, or WorkRuntime operation truth directly in command functions.

## Done Criteria

- Command additions or shape changes have matching Specta registration and
  binding drift checks.
- Focused command tests or owning module tests prove the behavior moved through
  the intended boundary.
- Direct review commands from `README.md` and `scripts/AGENTS.md` are the
  default verification path for command, contract, or generated-binding changes.
