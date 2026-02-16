---
name: contract-guardrails
description: Contract safety for audiobook-boss. Use when touching command signatures or changing TypeScript/Rust IPC boundaries to keep versions and contracts in sync.
---

# Contract Guardrails

Follow these steps to avoid version drift and TS/Rust contract mismatches.

## Required Steps

1) Do not bump version or edit `CHANGELOG.md` unless explicitly preparing a release.
2) Keep version sources in sync: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`.
3) After changing commands or payloads, run `scripts/checks.sh standard` (includes generated-binding drift checks).

## Optional Checks (Before Merge)

Standard tier (default for all workflows):

```bash
scripts/checks.sh standard
```

Package tier (validates app bundling):

```bash
scripts/checks.sh package
```

## Codebase Pointers

- `scripts/check-generated-bindings.sh`
- `src-tauri/src/ipc_contract.rs` (command and event registration)
