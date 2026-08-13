# Tauri Specta Route Card

- ABB crate: `tauri-specta`
- Upstream: `https://github.com/specta-rs/tauri-specta.git`
- Context7: none pinned; prefer `docs.rs` at the exact crate version. Current
  Context7/docs snapshots may lag ABB's pinned RC.

Resolve the pinned version from `Cargo.lock` (do not cache it here).

## Use For

- Tauri command/event export behavior, builder setup, language output, and
  integration between Tauri and Specta.
- Confirming how command/event bindings should be emitted and consumed.

## Installed / registry entrypoints

- Cargo registry crate `tauri-specta` at the `Cargo.lock` version and checksum
- `docs.rs/tauri-specta/<exact-version>`

## Exceptional upstream areas

- `src/lib.rs`, `src/builder.rs`, `src/commands.rs`, `src/event.rs`,
  `src/lang/js_ts.rs`
- `tests/tauri_command.rs`, `tests/test.rs`
- `examples/app/src-tauri/src/main.rs`
- `examples/app/src/bindings-ts-files`
- `macros/`

## Avoid

- Do not let upstream examples override ABB's runtime-boundary ownership.
- Do not change generated binding shape without proving TS/Rust parity.
- Do not treat Context7 or default-branch docs as proof of ABB's pinned
  release candidate.

## ABB Reconciliation

- Check `src-tauri/Cargo.toml` and `Cargo.lock` for the pinned
  `tauri-specta` version and features.
- Compare generated files with `src/lib/generated/tauri.ts`.
- Use `bun run bindings:check` and focused contract tests for any binding
  shape change.
