# Rust Core Crates

## Ownership

- `abb-*-core` crates package pure domain logic for existing ABB owners.
- They are package boundaries, not new Grey-Box Public APIs.
- Keep runtime adapters, IPC, filesystem, keychain, Tauri plugins, and FFmpeg in
  `src-tauri`.

## Dependency Rule

Core crates must not depend on:

- `tauri`
- `tauri-specta`
- `ffmpeg-next`
- `ffmpeg-sys-next`
- `keyring`
- Tauri plugins

## Direct Tests

- Audible: `cargo nextest run -p abb-audible-core`
- Metadata: `cargo nextest run -p abb-metadata-core`
- Media contract: `cargo nextest run -p abb-media-core`
- Output Artifact: `cargo nextest run -p abb-output-artifact-core`
- Processing: `cargo nextest run -p abb-processing-core`
- Remote Source: `cargo nextest run -p abb-remote-source-core`
- All core crates: run the package commands above sequentially.

Move tests with the pure logic. Use `src-tauri` tests only for adapters,
contracts, filesystem behavior, and media/runtime behavior.
