---
name: contract-guardrails
description: Tauri runtime boundary and TS/Rust IPC contract safety for Audiobook Boss. Use when adding/refactoring Tauri commands, command payloads, event wiring, runtime plugin adapters, generated bindings, or frontend tauriClient calls.
---

# Tauri Runtime Boundary / IPC Contract Guardrails

Use this skill when touching Tauri commands/events, runtime plugin adapters, or
the TS client boundary. It owns ABB's command conventions and generated-binding
parity in one place.

## Boundary Path

1. Define commands in `src-tauri/src/commands/*.rs` with `#[tauri::command]`.
2. Validate inputs at command ingress and return `crate::errors::Result<T>`.
3. Register commands/events in `src-tauri/src/ipc_contract.rs`.
4. Route frontend command/event/plugin use through `src/lib/tauri/client.ts`.
5. Regenerate or check bindings by risk.

## Command Skeleton

```rust
use crate::errors::{AppError, Result};

#[tauri::command]
pub async fn my_command(payload: MyPayload) -> Result<MyResult> {
    if payload.input.is_empty() {
        return Err(AppError::InvalidInput("input is required".into()));
    }
    Ok(MyResult::default())
}
```

## Required Workflow

1. Keep these version sources synchronized only during explicit release work:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
2. For command/event, payload, or TS adapter changes, verify contract parity with focused
   boundary checks first:
```bash
bun run bindings:check:local
bun run test -- src/lib/tauri-public-api.contract.test.ts src/lib/tauri-client.test.ts src/lib/tauri-client.generated-event-bindings.test.ts
scripts/check-public-api-strips.sh
```
3. Run `scripts/checks.sh standard` when the change affects Rust command/event
   signatures, generated bindings, runtime behavior, dependency/build/test
   semantics, or release-critical merge confidence.
4. For release-critical binding drift checks:
```bash
bun run bindings:check
# or
CHECK_BINDINGS_STRICT=1 scripts/checks.sh standard
```

## Command Pointers

- IPC contract registration: `src-tauri/src/ipc_contract.rs`
- Command handlers: `src-tauri/src/commands/`
- Error boundary: `src-tauri/src/errors.rs`
- TS runtime adapter boundary: `src/lib/tauri/client.ts`
- Generated TS bindings: `src/lib/generated/tauri.ts`
- Binding drift guard: `scripts/check-generated-bindings.sh`

## Done Criteria

- Rust commands/events compile, remain registered, and are callable through the
  TS boundary adapter when exposed to the frontend.
- Generated TS bindings match Rust contract.
- Errors map to stable `AppError` variants.
- Progress-stage authority is not duplicated here; use
  `job-registry-and-progress` and `src/types/events.ts` for lifecycle state.
- Verification follows root `AGENTS.md` risk-based scope: focused boundary
  checks for local adapter changes; `scripts/checks.sh standard` for contract,
  runtime, build/test, dependency, or release-critical risk.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
- For external Tauri/API uncertainty, invoke `abb-library-research`.
