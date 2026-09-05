# Tauri Specta

Read for command/event export, builder configuration, or Tauri/Specta integration.

- Resolve `tauri-specta` from `Cargo.lock`; inspect selected features in
  `src-tauri/Cargo.toml`.
- Prefer exact Cargo registry source and
  `docs.rs/tauri-specta/<exact-version>`. The packaged
  `.cargo_vcs_info.json`, when present, helps identify its source commit.
- Upstream: [tauri-specta](https://github.com/specta-rs/tauri-specta).
  Builder, command, event, and language-export implementation lives under
  `src/`; use the verified revision to locate its tests/examples.
- If using Context7, resolve the library ID live and check the indexed version.
  Default-branch or mismatched RC material is orientation only.

Compare findings with `src-tauri/src/ipc_contract.rs` and
`src/lib/generated/tauri.ts`. Applying a binding-shape change follows the
command/Tauri owner guidance and `scripts/AGENTS.md` proof routes.
