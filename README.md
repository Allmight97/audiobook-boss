# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC to optimized M4B audiobooks
- **Smart metadata** — series, narrator, cover art with Audiobookshelf/Apple Books dual-write compatibility
- **Parallel processing** with real-time progress and per-job cancellation
- **Metadata lookup** — search online databases and apply results in batch
- **Drag & drop** workflow — import files, edit tags, process, done

## Quick start

```bash
# Install dependencies
mise install

# Run in development
bun run tauri dev
```

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## For contributors & AI agents

This is a personal tool with a public repo. Contributions welcome but not expected.

- **Stack**: Rust (ffmpeg-next, mp4ameta) + TypeScript + Tauri 2 + Svelte
- **Architecture**: See `docs/external-apis/` for boundary docs
- **Agent guide**: `AGENTS.md` defines coding standards and workflows
- **Quality gates**: `scripts/checks.sh standard` before PRs

## Development

```bash
scripts/checks.sh standard    # Full quality gate
bun run tauri dev             # Dev mode
bun run test                  # All tests
```

**[Full technical reference →](docs/specs/technical-reference.md)** — architecture, data flows, IPC contracts, coding standards.
