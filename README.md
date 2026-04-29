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
- Performance: `bun run perf`, `bun run perf:quick`, `bun run perf:real`, `bun run perf:audio`, `bun run perf:list`
- xHE-AAC fixture proof: `ABB_XHE_AAC_FIXTURE=/path/to/book.m4b cargo test -p audiobook-boss --test integration_xhe_aac_fixture_tests -- --ignored`
  Optionally set `ABB_XHE_AAC_FFMPEG=/path/to/ffmpeg` to validate a specific FDK-capable external FFmpeg. The fixture is local-only and not committed.
- Release: `bun run release:notes`, `bun run release:run`
  `bun run app:build` remains the direct local `.app` path.
  `bun run release:run` now preflights the DMG release artifact.
  GitHub Releases are published from pushed `v*` tags and use the matching
  `CHANGELOG.md` section as the release body and attach the versioned DMG.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- For substantial multi-step work, use [PLANS.md](PLANS.md) and keep at most active implementation specs under `docs/specs/`; these specs are working docs and are deleted when the effort is fully done.
- For the product/system shape, use [docs/system-map.md](docs/system-map.md) and [docs/ubiquitous-language.md](docs/ubiquitous-language.md).
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Cheap deterministic repo guardrails live in `.codex/hooks.json` and `./.agents/hooks/`; `.codex` is a tracked symlink to `.agents`.
- Durable truth lives in code, GitHub issues, this file, and [AGENTS.md](AGENTS.md). `.artifacts/` is temporary local state only.
