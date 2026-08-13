# Tauri Plugins Route Card

- ABB packages/crates: `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`,
  `tauri-plugin-dialog`, `tauri-plugin-opener`
- Upstream: `https://github.com/tauri-apps/plugins-workspace.git`
- Context7: `/tauri-apps/tauri-plugin-shell` for shell; prefer installed
  package docs for dialog and opener

Resolve installed versions from `bun.lock` and `Cargo.lock` (do not cache them
here).

## Use For

- Tauri v2 plugin behavior, guest JS APIs, Rust plugin commands, permission
  schemas, and plugin-specific examples.
- ABB-installed plugins such as dialog and opener.

## Installed / registry entrypoints

- `node_modules/@tauri-apps/plugin-dialog`
- `node_modules/@tauri-apps/plugin-opener`
- Cargo registry crates `tauri-plugin-dialog` and `tauri-plugin-opener` at the
  `Cargo.lock` versions and checksums
- `docs.rs/tauri-plugin-dialog/<exact-version>`
- `docs.rs/tauri-plugin-opener/<exact-version>`

## Exceptional upstream areas

- `plugins/dialog/guest-js`, `plugins/dialog/src`, `plugins/dialog/permissions`
- `plugins/opener/guest-js`, `plugins/opener/src`, `plugins/opener/permissions`
- `plugins/dialog/test`, `plugins/*/permissions/schemas`

## Avoid

- Do not treat every plugin in the workspace as an ABB dependency.
- Do not copy permission files without comparing ABB capabilities.
- Import installed `@tauri-apps/plugin-*` packages, not unpublished guest JS.

## ABB Reconciliation

- Check `package.json`, `bun.lock`, and `src-tauri/Cargo.toml` for installed
  plugin versions.
- Compare plugin permission requirements with `src-tauri/capabilities`.
- Keep frontend plugin calls behind `src/lib/tauri/*` where ABB already owns the
  runtime boundary.
