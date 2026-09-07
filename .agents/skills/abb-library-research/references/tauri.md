# Tauri

Read for runtime, command/event, capability, WebView, config, or bundling questions.

- JS packages: `@tauri-apps/api`, `@tauri-apps/cli` from `bun.lock`.
- Rust crates: `tauri`, `tauri-build`, `tauri-utils` from `Cargo.lock`.
- Inspect installed JS exports and the selected Cargo registry crates;
  `docs.rs/<crate>/<exact-version>` supplies versioned public API docs.
- Upstream: [Tauri](https://github.com/tauri-apps/tauri), with JS in
  `packages/api` and Rust under `crates/`.
- Optional Context7 hint: `/websites/v2_tauri_app`.

For command/event questions, compare `src-tauri/src/ipc_contract.rs`,
`src/lib/generated/tauri.ts`, and `src/lib/tauri/client.ts`. The command
and frontend Tauri `AGENTS.md` files own adaptation rules. For permission
or bundle questions, compare `src-tauri/capabilities` and
`src-tauri/tauri.conf.json` before adopting upstream examples.
