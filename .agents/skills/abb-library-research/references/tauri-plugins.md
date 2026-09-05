# Tauri Plugins

Read for an installed plugin's guest API, Rust behavior, or permissions.

Identify the plugin in `package.json` and `src-tauri/Cargo.toml`, then
resolve both sides from `bun.lock` and `Cargo.lock`. Inspect its installed
`@tauri-apps/plugin-*` package and exact `tauri-plugin-*` Cargo crate.

[plugins-workspace](https://github.com/tauri-apps/plugins-workspace) contains
`plugins/<name>/guest-js`, `src`, and `permissions`. Its other plugins are
not automatically ABB dependencies. Prefer exact package docs or
`docs.rs/<crate>/<exact-version>` for a public API question.

Compare permission requirements with `src-tauri/capabilities`. Apply
frontend changes through the runtime boundary owned by
`src/lib/tauri/AGENTS.md`.
