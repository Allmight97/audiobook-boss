# Tauri Route Card

- ABB packages/crates: `@tauri-apps/api`, `@tauri-apps/cli`, `tauri`,
  `tauri-build`, `tauri-utils`
- Upstream: `https://github.com/tauri-apps/tauri.git`
- Context7: `/websites/v2_tauri_app`

Resolve installed versions from `bun.lock` and `Cargo.lock` (do not cache them
here). Do not treat upstream `dev` as ABB truth until those versions match.

## Use For

- Tauri runtime behavior, command/event APIs, JS API shapes, capabilities,
  window/webview behavior, config, build, and bundling details.
- Source-level confirmation when Context7/current docs leave contract behavior
  ambiguous.

## Installed / registry entrypoints

- `node_modules/@tauri-apps/api`
- Cargo registry crate `tauri` at the `Cargo.lock` version and checksum
- `docs.rs/tauri/<exact-version>`
- `docs.rs/tauri-utils/<exact-version>`

## Exceptional upstream areas

- `packages/api/src`
- `crates/tauri/src`
- `crates/tauri-runtime/src`
- `crates/tauri-runtime-wry/src`
- `crates/tauri-utils/src/config`
- `crates/tauri-bundler/src`
- `crates/tests`, `crates/tauri/test`, `examples/api`, `examples/commands`

## Avoid

- Do not bypass ABB's centralized runtime boundary in `src/lib/tauri/*`.
- Do not copy Tauri example permission/config shapes without comparing ABB's
  `src-tauri/capabilities` and `src-tauri/tauri.conf.json`.

## ABB Reconciliation

- Check `package.json`, `bun.lock`, `src-tauri/Cargo.toml`, and `Cargo.lock`
  for installed Tauri versions.
- Validate TS/Rust contract changes against `src/lib/generated/tauri.ts` and
  `src-tauri/src/ipc_contract.rs`.
- For command, event, binding, or runtime-boundary edits, see
  `src-tauri/src/commands/AGENTS.md` and `src/lib/tauri/AGENTS.md`.
