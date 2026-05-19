# Tauri Plugins Route Card

- Upstream: `https://github.com/tauri-apps/plugins-workspace.git` on `v2`
- Local subtree: `repos/tauri-plugins`

Refresh:

```bash
git subtree pull --prefix=repos/tauri-plugins https://github.com/tauri-apps/plugins-workspace.git v2 --squash
```

## Use For

- Tauri v2 plugin behavior, guest JS APIs, Rust plugin commands, permission
  schemas, and plugin-specific examples.
- ABB-installed plugins such as dialog and opener.

## Start Here

- `repos/tauri-plugins/plugins/dialog/guest-js`
- `repos/tauri-plugins/plugins/dialog/src`
- `repos/tauri-plugins/plugins/dialog/permissions`
- `repos/tauri-plugins/plugins/opener/guest-js`
- `repos/tauri-plugins/plugins/opener/src`
- `repos/tauri-plugins/plugins/opener/permissions`

## Examples And Tests

- `repos/tauri-plugins/plugins/dialog/test`
- `repos/tauri-plugins/plugins/shell/test`
- `repos/tauri-plugins/examples`
- `repos/tauri-plugins/plugins/*/permissions/schemas`

## Avoid

- Do not treat every plugin in the workspace as an ABB dependency.
- Do not copy permission files without comparing ABB capabilities.
- Do not import guest JS from this subtree; ABB imports installed
  `@tauri-apps/plugin-*` packages.

## ABB Reconciliation

- Check `package.json`, `bun.lock`, and `src-tauri/Cargo.toml` for installed
  plugin versions.
- Compare plugin permission requirements with `src-tauri/capabilities`.
- Keep frontend plugin calls behind `src/lib/tauri/*` where ABB already owns the
  runtime boundary.
