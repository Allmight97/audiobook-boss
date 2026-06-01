# Rust Core Crates

## Ownership

- `abb-*-core` crates package pure domain logic for existing ABB owners.
- They are proof/package boundaries, not new Grey-Box Public APIs.
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

## Proof

- Metadata: `bun scripts/proof/runner.ts focus core metadata`
- Output Artifact: `bun scripts/proof/runner.ts focus core output-artifact`
- Processing: `bun scripts/proof/runner.ts focus core processing`
- Remote Source: `bun scripts/proof/runner.ts focus core remote-source`
- All core crates: `bun scripts/proof/runner.ts review core`

Move tests with the pure logic. Use `src-tauri` tests only for adapters,
contracts, filesystem behavior, and media/runtime proof.
