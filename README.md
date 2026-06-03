# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC/WAV/FLAC to optimized M4B audiobooks
- **Book Binder** — Merge multiple chapterized audio files into a single M4B with chapter markers and metadata.
- **Smart metadata** — series, narrator, cover art with Audiobookshelf/Apple Books dual-write compatibility
- **Parallel processing** with real-time progress and per-job cancellation
- **Metadata lookup** — search online databases and apply results in batch
- **Drag & drop** workflow — import files, edit tags, process, done

## Quick start

```bash
# System dependencies (macOS)
brew install ffmpeg

# Install JS/TS dependencies
bun install

# Run in development
bun run tauri dev
```

**AAC runtime contract**: output encoder choice and input decoder choice are separate concerns. Normal processing uses the in-process `ffmpeg-next` engine. FDK HE-AAC output routes through an external FFmpeg/libfdk_aac processor adapter, which can force compatible AAC-family input decoders such as `aac_at` or `libfdk_aac` when the default decoder cannot handle the source. For higher quality AAC encoding and broader AAC decode compatibility on macOS, `brew install fdk-aac` and rebuild ffmpeg with `--enable-libfdk-aac`.

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Toolchain

- Package manager: **Bun 1.3.14 stable**.
- Refresh: `bun upgrade --stable`.
- Verify Bun changes with direct test/build commands; use `.agents/skills/release`
  for packaging/release work.

## Development

```bash
bun install
bun run tauri dev
```

Run targeted validation from the script guide for the owner or risk surface you
touched.

## Script Guide

Human index for common commands. `package.json` owns shortcuts and
`scripts/AGENTS.md` owns the exact fresh-agent verification command menu. ABB
currently uses direct native commands, not a custom verification runner or a
default full review matrix.

- Core dev: `bun run tauri dev`, `bun run build`
- Verification is owner-scoped by default. Run the smallest native command that
  proves the touched owner or risk surface, then escalate only when the change
  crosses owners or a concrete safety/data/contract invariant requires it.
- Focused Rust loops:
  `cargo nextest run -p abb-media-core`,
  `cargo nextest run -p abb-metadata-core`,
  `cargo nextest run -p abb-output-artifact-core`,
  `cargo nextest run -p abb-processing-core`,
  `cargo nextest run -p abb-remote-source-core`,
  `cargo nextest run -p audiobook-boss --lib`, or
  `cargo nextest run -p audiobook-boss --test all_tests`.
- Focused frontend loops: `bun run test -- <test files>`.
- Policy checks: `scripts/check-generated-bindings.sh --mode local`,
  `scripts/check-fallback-policy.sh`, and `scripts/check-no-bridge-imports.sh`.
  Run standalone when touching those rule sets directly.
- Dependency hygiene: `bun run audit`
  It is not part of the normal review matrix.
- Tooling policy: Bun is the package manager/script runner/test runner.
  Keep Vite scripts on the standard Vite CLI unless a validated tooling
  decision changes that.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- Build timing: use direct Cargo timing commands such as `cargo build --timings`
  when investigating compile cost.
- Release: use `.agents/skills/release`.
  `scripts/bump-version.sh <version>` updates version surfaces;
  `bun run app:build` builds a repo-local `.app`; `bun run app:install-local`
  installs `/Applications/AudioBook Boss.app`; `bun run app:build:dmg` builds a DMG.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the release artifact before publishing a GitHub Release.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- Grey-box module work is governed by the eight Public APIs documented in [docs/ubiquitous-language.md](docs/ubiquitous-language.md): Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, Status Panel Runtime, Audio Engine Deep Module, App Settings, and RemoteSourceRuntime.
- For substantial planning, roadmap, architecture, or implementation alignment, use `.agents/skills/decision-alignment` to align outcomes with the repo owner. Keep active specs under `docs/specs/` only while the work needs a temporary repo-carried plan; delete them or distill enduring truths into canon when done. External presentation artifacts belong under `/Users/jstar/Documents/Codex/artifacts/audiobook-boss`.
- For external library/API behavior, use `.agents/skills/abb-library-research` as the control plane for authenticated Context7/current docs, squashed `repos/*` reference source, route cards, subtree refresh guidance, and installed dependency truth. Reference repos are read-only research material, not app dependencies.
- For the product/system shape, use [docs/system-map.md](docs/system-map.md) and [docs/ubiquitous-language.md](docs/ubiquitous-language.md).
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Durable truth lives in code, GitHub issues, this file, and [AGENTS.md](AGENTS.md). `.artifacts/` is temporary local state only.
