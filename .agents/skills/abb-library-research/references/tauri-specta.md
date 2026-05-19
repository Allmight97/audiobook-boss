# Tauri Specta Route Card

- Upstream: `https://github.com/specta-rs/tauri-specta.git` on `main`
- Local subtree: `repos/tauri-specta`

Refresh:

```bash
git subtree pull --prefix=repos/tauri-specta https://github.com/specta-rs/tauri-specta.git main --squash
```

## Use For

- Tauri command/event export behavior, builder setup, language output, and
  integration between Tauri and Specta.
- Confirming how command/event bindings should be emitted and consumed.

## Start Here

- `repos/tauri-specta/src/lib.rs`
- `repos/tauri-specta/src/builder.rs`
- `repos/tauri-specta/src/commands.rs`
- `repos/tauri-specta/src/event.rs`
- `repos/tauri-specta/src/lang/js_ts.rs`

## Examples And Tests

- `repos/tauri-specta/tests/tauri_command.rs`
- `repos/tauri-specta/tests/test.rs`
- `repos/tauri-specta/examples/app/src-tauri/src/main.rs`
- `repos/tauri-specta/examples/app/src/bindings-ts-files`
- `repos/tauri-specta/examples/custom-plugin`

## Avoid

- Do not let upstream examples override ABB's runtime-boundary ownership.
- Do not change generated binding shape without proving TS/Rust parity.
- Do not assume upstream `main` matches ABB's pinned `tauri-specta` release
  candidate.

## ABB Reconciliation

- Check `src-tauri/Cargo.toml` and `Cargo.lock` for the pinned
  `tauri-specta` version and features.
- Compare generated files with `src/lib/generated/tauri.ts`.
- Use `bun run bindings:check` and focused contract tests for any binding
  shape change.
