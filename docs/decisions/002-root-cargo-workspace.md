# ADR-002: Root Cargo Workspace

**Status:** accepted
**Date:** 2026-01-11
**Issue:** N/A

## Context
Agents and contributors sometimes run `cargo` commands from the repo root, which previously failed because the Rust crate lived in `src-tauri/`. This caused avoidable friction and inconsistent check instructions.

## Decision
Create a root Cargo workspace that includes `src-tauri`, making cargo commands work from the repo root. Update scripts and documentation to run Rust checks from root and relocate `Cargo.lock` to the workspace root.

## Consequences
### Pros
- `cargo fmt`, `cargo clippy`, and `cargo test` work from the repo root.
- Fewer agent mistakes and less “cd into src-tauri” foot-gun friction.
- Scripts and docs become simpler and consistent.

### Cons
- `Cargo.lock` now lives at the repo root (workspace default).
- Root workspace becomes the single source of truth for Rust tooling.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep Rust crate isolated in `src-tauri/` and only update scripts/docs | Still fragile if agents run cargo commands directly from root. |
| Require `--manifest-path src-tauri/Cargo.toml` everywhere | Easy to forget; less discoverable than a workspace default. |
| Add wrapper scripts only | Helps when used, but doesn’t fix direct cargo usage. |
