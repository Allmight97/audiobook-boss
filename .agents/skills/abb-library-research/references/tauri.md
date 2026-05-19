# Tauri Route Card

- Upstream: `https://github.com/tauri-apps/tauri.git` on `dev`
- Local subtree: `repos/tauri`

Refresh:

```bash
git subtree pull --prefix=repos/tauri https://github.com/tauri-apps/tauri.git dev --squash
```

## Use For

- Tauri runtime behavior, command/event APIs, JS API shapes, capabilities,
  window/webview behavior, config, build, and bundling details.
- Source-level confirmation when Context7/current docs leave contract behavior
  ambiguous.

## Start Here

- `repos/tauri/packages/api/src`
- `repos/tauri/crates/tauri/src`
- `repos/tauri/crates/tauri-runtime/src`
- `repos/tauri/crates/tauri-runtime-wry/src`
- `repos/tauri/crates/tauri-utils/src/config`
- `repos/tauri/crates/tauri-bundler/src`

## Examples And Tests

- `repos/tauri/examples/api/src`
- `repos/tauri/examples/api/src-tauri`
- `repos/tauri/examples/commands`
- `repos/tauri/crates/tests`
- `repos/tauri/crates/tauri/test`

## Avoid

- Do not bypass ABB's centralized runtime boundary in `src/lib/tauri/*`.
- Do not copy Tauri example permission/config shapes without comparing ABB's
  `src-tauri/capabilities` and `src-tauri/tauri.conf.json`.
- Do not treat Tauri `dev` branch behavior as installed ABB behavior until
  package and Cargo versions match.

## ABB Reconciliation

- Check `package.json`, `bun.lock`, `src-tauri/Cargo.toml`, and `Cargo.lock`
  for installed Tauri versions.
- Validate TS/Rust contract changes against `src/lib/generated/tauri.ts` and
  `src-tauri/src/ipc_contract.rs`.
- Use `contract-guardrails` for command, event, binding, or runtime-boundary
  edits.
