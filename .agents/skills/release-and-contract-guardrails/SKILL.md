---
name: release-and-contract-guardrails
description: Release and contract safety for audiobook-boss. Use when preparing releases, touching command signatures, or changing TypeScript/Rust IPC boundaries to keep versions and contracts in sync.
---

# Release and Contract Guardrails

Follow these steps to avoid version drift and TS/Rust contract mismatches.

## Required Steps

1) Do not bump version or edit `CHANGELOG.md` unless explicitly preparing a release.
2) For releases, use `scripts/release.sh` (it bumps versions and handles tagging).
3) Keep version sources in sync: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`.
4) After changing commands or payloads, run `scripts/ensure-contract.sh`.

## Optional Checks (Before Merge/Release)

Quick tier (pre-commit / iteration):

```bash
scripts/quick-checks.sh
```

Standard tier (pre-push / PR readiness):

```bash
scripts/standard-checks.sh
```

Release tier (pre-release only):

```bash
scripts/release-checks.sh
```

## Codebase Pointers

- `scripts/release.sh`
- `scripts/bump-version.sh`
- `scripts/ensure-contract.sh`
- `src-tauri/src/main.rs` (command registration)

