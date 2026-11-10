# Rust Crates

## Tiers

| Tier | Crates | Holds | Must not depend on |
| --- | --- | --- | --- |
| Pure cores | `abb-*-core` | Domain facts and classifiers with no I/O. | A UI toolkit, FFmpeg, a credential store, or the engine. |
| FFmpeg layer | `abb-ffmpeg` | Every call into the FFmpeg C API (`ffmpeg-sys-next`), offered as owned values and typed errors. It holds operations, never a product rule. | Any other ABB crate. |
| Engine | `abb-engine` | Every product rule and workflow: the working session, settings, audio, metadata, processing, WorkRuntime, and remote sources. | A UI toolkit (`tauri`, `tauri-*`, `wry`, `tao`). |
| Host | `src-tauri` (package `audiobook-boss`) | One UI host: commands, events, the window, native dialogs. | — |

`bun run check:rust-tiers` (core lane) enforces the last column;
`scripts/check-known-mistakes.ts` rejects I/O in a core's source.

Only `abb-ffmpeg` names `ffmpeg_sys_next`; the same check rejects it in any
other crate's source.

## Ownership

- `abb-*-core` crates are package boundaries, not additional Public API Strips.
- `abb-remote-source-core` stays provider-neutral (stages, progress;
  `check-known-mistakes.ts` rejects a provider name there). Provider protocol
  interpretation, such as Audible license keys and strategy choice, lives in
  that provider's core.
- Engine owners and the engine's host interface: `crates/abb-engine/AGENTS.md`.
- `abb-ffmpeg` rules: `crates/abb-ffmpeg/AGENTS.md`.
- Host rules: `src-tauri/AGENTS.md`.
