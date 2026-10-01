# Rust Crates

## Tiers

| Tier | Crates | Holds | Must not depend on |
| --- | --- | --- | --- |
| Pure cores | `abb-*-core` | Domain facts and classifiers with no I/O. | A UI toolkit, FFmpeg, a credential store, or the engine. |
| Engine | `abb-engine` | Every product rule and workflow: the working session, settings, audio, metadata, processing, WorkRuntime, and remote sources. | A UI toolkit (`tauri`, `tauri-*`, `wry`, `tao`). |
| Host | `src-tauri` (package `audiobook-boss`) | One UI host: commands, events, the window, native dialogs. | — |

`bun run check:rust-tiers` enforces the last column and runs in CI.

A UI host on another platform replaces the host tier and keeps the engine, so
a rule placed in the host would have to be written again for each platform.
Put a rule in the engine, or in a core when it needs no I/O.

## Ownership

- `abb-*-core` crates package pure domain logic for an engine owner. They are
  package boundaries, not additional Public API Strips.
- `abb-remote-source-core` stays provider-neutral (stages, strategies,
  progress, materialized kinds). Provider protocol interpretation, such as
  Audible license keys and strategy choice, lives in that provider's core.
- Engine owners and the engine's host interface: `crates/abb-engine/AGENTS.md`.
- Host rules: `src-tauri/AGENTS.md`.

## Direct Tests

- A core: `cargo nextest run -p abb-<owner>-core` (audible, media, metadata,
  output-artifact, processing, remote-source). Run several sequentially.
- Engine unit tests: `cargo nextest run -p abb-engine --features bundled-ffmpeg --lib`
- Engine real-file tests: `cargo nextest run -p abb-engine --features bundled-ffmpeg --test all_tests`
- Host: `cargo nextest run -p audiobook-boss --features bundled-ffmpeg`

Move a test with the logic it proves; what the host's own tests cover is in
`src-tauri/AGENTS.md`.
