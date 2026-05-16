---
name: contract-guardrails
description: Contract safety for TS/Rust IPC changes. Use when command signatures, payloads, event types, or generated bindings may drift.
---

# Contract Guardrails

Use this skill when touching Tauri commands/events or TS client boundary contracts.

## Required Workflow

1. Keep these version sources synchronized only during explicit release work:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
2. For command/event or payload changes, verify contract parity with focused
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
- TS runtime adapter boundary: `src/lib/tauri/client.ts`
- Binding drift guard: `scripts/check-generated-bindings.sh`

## Done Criteria

- Rust commands/events compile and remain registered.
- Generated TS bindings match Rust contract.
- Verification follows root `AGENTS.md` risk-based scope: focused boundary
  checks for local adapter changes; `scripts/checks.sh standard` for contract,
  runtime, build/test, dependency, or release-critical risk.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
