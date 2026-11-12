# Specta

Read for Rust type export, serde mapping, macros, or TypeScript generation.

- Resolve `specta` and `specta-typescript` from `Cargo.lock`, including
  their source/checksum. Root `Cargo.toml` `[workspace.dependencies]` pins
  the Specta family once, with the pin's reason; each crate manifest adds
  its features.
- Use the selected Cargo registry sources or
  `docs.rs/<crate>/<exact-version>`. Default-branch release-candidate docs
  may differ from ABB's pinned release.
- Upstream: [Specta](https://github.com/specta-rs/specta), including
  `specta`, `specta-typescript`, `specta-macros`, and tests/examples.

Compare research with ABB's exporter and `src/lib/generated/tauri.ts`.
If the answer changes TS/Rust shape, follow
`src-tauri/AGENTS.md`, `src/lib/tauri/AGENTS.md`, and prove it with
`bun run bindings:check` and the `host` lane. Research alone does
not require regeneration.
