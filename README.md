# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC to optimized M4B audiobooks
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

**AAC runtime contract**: output encoder choice and input decoder choice are separate concerns. The app stays on the single `ffmpeg-next` engine, but may select compatible AAC decoders such as `aac_at` or `libfdk_aac` at runtime for AAC-family inputs that the default decoder cannot handle. For higher quality AAC encoding and broader AAC decode compatibility on macOS, `brew install fdk-aac` and rebuild ffmpeg with `--enable-libfdk-aac`.

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Development

```bash
bun install
bun run tauri dev
scripts/checks.sh standard
bun run test
bash scripts/check-context-surface.sh
bun run harness:verify --changed
```

## Script Guide

Use this section as the human-facing index. `package.json` is the source of truth for `bun run ...` entrypoints, and the scripts under `scripts/` are the source of truth for flags and implementation details.

- Core dev: `bun run tauri dev`, `bun run build`, `bun run test`
- Main quality gate: `scripts/checks.sh standard`
  Use `quick` for a faster local pass and `package` when validating the full Tauri packaging path.
- Context and policy checks: `bun run check:context`, `bun run check:fallback`, `bun run check:no-bridge`
- Dependency hygiene: `bun run check:deps`
  This is explicit on purpose and is not part of the normal standard gate.
- UI proof loop: `bun run harness:verify --changed`
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- Performance: `bun run perf`, `bun run perf:quick`, `bun run perf:real`, `bun run perf:audio`, `bun run perf:list`
- Release: `bun run release:notes`, `bun run release:run`
- Offline handoff: `bun run repomix:audit`, `bun run repomix:full`

## Project Operation

- Agents: start in [AGENTS.md](/Users/jstar/Projects/audiobook-boss/AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- UI work is not done from static inspection alone. Run `bun run harness:verify --changed` for UI-affecting changes.
- Cheap deterministic repo guardrails live in `hooks.json` and `./.agents/hooks/`.
- Durable truth lives in code, GitHub issues and PRs, this file, and [AGENTS.md](/Users/jstar/Projects/audiobook-boss/AGENTS.md). `.artifacts/` is temporary local state only.
