# Specta Route Card

- ABB crates: `specta`, `specta-typescript`
- Upstream: `https://github.com/specta-rs/specta.git`
- Context7: none pinned; prefer `docs.rs` at the exact crate version

Resolve pinned versions from `Cargo.lock` (do not cache them here). ABB pins
release candidates; upstream default branch is not proof.

## Use For

- Rust type export behavior, TypeScript generation, serde/type mapping, macro
  behavior, and generated binding expectations.
- Source-level confirmation for ABB's IPC type export path.

## Installed / registry entrypoints

- Cargo registry crates `specta` and `specta-typescript` at the `Cargo.lock`
  versions and checksums
- `docs.rs/specta/<exact-version>`
- `docs.rs/specta-typescript/<exact-version>`

## Exceptional upstream areas

- `specta/src`
- `specta-typescript/src`
- `specta-macros/src`
- `specta-serde/src`
- `tests/tests`, `tests/tests/macro`, `tests/tests/snapshots`
- `examples/basic-ts`, `examples/collect`

## Avoid

- Do not assume upstream default-branch docs match ABB's pinned release
  candidates.
- Do not hand-edit generated ABB bindings to match upstream examples.
- Do not copy snapshot expectations without checking ABB's exporter command.

## ABB Reconciliation

- Check `src-tauri/Cargo.toml` and `Cargo.lock` for pinned `specta` and
  `specta-typescript` versions.
- Verify generated output through `bun run bindings:check` or the relevant
  focused binding command before changing IPC contracts.
- When Specta output affects TS/Rust parity, see `src-tauri/src/commands/AGENTS.md` and `src/lib/tauri/AGENTS.md`.
