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

# Required Node runtime for Node-backed CLIs: Node 24.x LTS.
# Use a node manager that honors .node-version or .nvmrc.

# Install JS/TS dependencies
bun install

# Run in development
bun run tauri dev
```

**AAC runtime contract**: output encoder choice and input decoder choice are separate concerns. Normal processing uses the in-process `ffmpeg-next` engine. FDK HE-AAC output routes through an external FFmpeg/libfdk_aac processor adapter, which can force compatible AAC-family input decoders such as `aac_at` or `libfdk_aac` when the default decoder cannot handle the source. For higher quality AAC encoding and broader AAC decode compatibility on macOS, `brew install fdk-aac` and rebuild ffmpeg with `--enable-libfdk-aac`.

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Development

```bash
bun install
bun run tauri dev
scripts/proof.sh standard
bun run test
```

## Script Guide

Use this section as the human-facing index. `scripts/proof.sh --help` is the canonical proof-routing surface for humans and agents. `package.json` remains the source of truth for `bun run ...` convenience entrypoints, and scripts under `scripts/` own their implementation details.

- Core dev: `bun run tauri dev`, `bun run build`, `bun run test`
- Main quality gate: `scripts/proof.sh standard`
  Use `quick` for static/boundary proof, `package` for the full Tauri packaging path, and `runtime`, `frontend`, `rust-contract`, `rust-private`, or `rust-media` for focused owner proof.
- Policy checks: `bun run check:fallback`, `bun run check:no-bridge`
  Public-strip drift is checked by `scripts/check-public-api-strips.sh`.
- Dependency hygiene: `bun run check:deps`
  Run explicitly, or use `scripts/proof.sh deps`; it is not part of the normal standard gate.
- Tooling policy: Bun is the package manager/script runner/test runner.
  `.node-version`, `.nvmrc`, and `package.json` engines require Node 24.x LTS
  for Node-backed CLIs such as Vite. Do not switch Vite scripts to `--bun`
  without a proof-backed tooling decision.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- xHE-AAC fixture proof: `ABB_XHE_AAC_FIXTURE=/path/to/book.m4b scripts/proof.sh rust-media-manual xhe-aac`
  Optionally set `ABB_XHE_AAC_FFMPEG=/path/to/ffmpeg` to validate a specific FDK-capable external FFmpeg. The fixture is local-only and not committed.
- Build timing: `scripts/proof.sh timing`
  Use timing proof for compile/build feedback; do not infer compile cost from late-stage spinner labels.
- Release: use `.agents/skills/release`.
  `scripts/bump-version.sh <version>` updates version surfaces.
  `bun run app:build` builds the repo-local `.app` artifact only.
  `bun run app:install-local` builds, installs, signs, registers, and indexes a real `/Applications/AudioBook Boss.app` for local Raycast/Finder testing, then removes the repo-local `.app` install artifact.
  `bun run app:build:dmg` builds a DMG.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the release artifact before publishing a GitHub Release.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- Grey-box module work is governed by the six Public APIs documented in [docs/ubiquitous-language.md](docs/ubiquitous-language.md): Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, Status Panel Runtime, and Audio Engine Deep Module.
- For substantial planning, roadmap, architecture, or implementation alignment, use `.agents/skills/decision-alignment` to align outcomes with the repo owner. Keep active specs under `docs/specs/` only while the work needs a temporary repo-carried plan; delete them or distill enduring truths into canon when done. External presentation artifacts belong under `/Users/jstar/Documents/Codex/artifacts/audiobook-boss`.
- For external library/API behavior, use `.agents/skills/abb-library-research` as the control plane for authenticated Context7/current docs, squashed `repos/*` reference source, route cards, subtree refresh guidance, and installed dependency truth. Reference repos are read-only research material, not app dependencies.
- For the product/system shape, use [docs/system-map.md](docs/system-map.md) and [docs/ubiquitous-language.md](docs/ubiquitous-language.md).
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Durable truth lives in code, GitHub issues, this file, and [AGENTS.md](AGENTS.md). `.artifacts/` is temporary local state only.
