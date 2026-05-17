---
name: tauri-command-conventions
description: Tauri command implementation rules for command signatures, state access, error mapping, and TS/Rust IPC boundary hygiene.
---

# Tauri Command Conventions

Use this skill when adding or changing Tauri commands, command payloads, or event wiring.

## Preferred Path

1. Define command in `src-tauri/src/commands/*.rs` with `#[tauri::command]`.
2. Validate inputs at command boundary and return `crate::errors::Result<T>`.
3. Register command/event in `src-tauri/src/ipc_contract.rs`.
4. Route frontend command/event usage through `src/lib/tauri/client.ts`.
5. Regenerate/check bindings and choose verification by risk: focused boundary
   checks for local adapter changes; `scripts/checks.sh standard` for command
   contracts, runtime behavior, build/test semantics, or release-critical work.

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

## Hard Invariants

- Command and event contracts remain registered in `ipc_contract::builder()`.
- TS/Rust contract parity is verified via generated bindings.
- Progress-stage authority is not duplicated here; use `job-registry-and-progress` skill + `src/types/events.ts` as canonical source.

## Verification

For focused TS boundary or generated-binding changes:

```bash
bun run bindings:check:local
bun run test -- src/lib/tauri-public-api.contract.test.ts src/lib/tauri-client.test.ts src/lib/tauri-client.generated-event-bindings.test.ts
scripts/check-public-api-strips.sh
```

For command contracts, runtime behavior, build/test semantics, or
release-critical work:

```bash
scripts/checks.sh standard
```

For release-critical contract checks:

```bash
bun run bindings:check
```

## Pointers

- `src-tauri/src/ipc_contract.rs`
- `src-tauri/src/errors.rs`
- `src/lib/tauri/client.ts`
- `src/lib/generated/tauri.ts`
- `scripts/check-generated-bindings.sh`

## Done Criteria

- Command compiles, is registered, and callable from TS boundary adapter.
- Errors map to stable `AppError` variants.
- Verification follows root `AGENTS.md` risk-based scope and proves the changed
  Tauri boundary surface.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
- For external API uncertainty, invoke `abb-library-research`.
