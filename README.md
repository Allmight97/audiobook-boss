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

## Development

```bash
bun install
bun run tauri dev
scripts/checks.sh standard
bun run test
bash scripts/check-context-surface.sh
```

## Script Guide

Use this section as the human-facing index. `package.json` is the source of truth for `bun run ...` entrypoints, and the scripts under `scripts/` are the source of truth for flags and implementation details.

- Core dev: `bun run tauri dev`, `bun run build`, `bun run test`
- Main quality gate: `scripts/checks.sh standard`
  Use `quick` for a faster local pass and `package` when validating the full Tauri packaging path.
- Context and policy checks: `bun run check:context`, `bun run check:fallback`, `bun run check:no-bridge`
- Dependency hygiene: `bun run check:deps`
  This is explicit on purpose and is not part of the normal standard gate.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- xHE-AAC fixture proof: `ABB_XHE_AAC_FIXTURE=/path/to/book.m4b cargo test -p audiobook-boss --test integration_xhe_aac_fixture_tests -- --ignored`
  Optionally set `ABB_XHE_AAC_FFMPEG=/path/to/ffmpeg` to validate a specific FDK-capable external FFmpeg. The fixture is local-only and not committed.
- Release: use `.agents/skills/release`.
  `scripts/bump-version.sh <version>` updates version surfaces.
  `bun run app:build` builds the local `.app`; `bun run app:build:dmg` builds a DMG.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the release artifact before manual publishing.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- Grey-box module work is governed by the five Public APIs documented in [docs/ubiquitous-language.md](docs/ubiquitous-language.md): Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Intent Plan, and Status Panel Runtime.
- For substantial multi-step work, use `.agents/skills/decision-alignment` to align outcomes with the repo owner and keep at most active implementation specs under `docs/specs/`; these specs are working docs and are deleted when the effort is fully done.
- For the product/system shape, use [docs/system-map.md](docs/system-map.md) and [docs/ubiquitous-language.md](docs/ubiquitous-language.md).
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Cheap deterministic repo guardrails live in `.codex/hooks.json` and `./.agents/hooks/`; `.codex` is a tracked symlink to `.agents`.
- Durable truth lives in code, GitHub issues, this file, and [AGENTS.md](AGENTS.md). `.artifacts/` is temporary local state only.
