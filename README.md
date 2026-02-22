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
# System dependencies (macOS)
brew install ffmpeg

# Install JS/TS dependencies
bun install

# Run in development
bun run tauri dev
```

**Encoders**: FFmpeg's native AAC works out of the box. For higher quality, `brew install fdk-aac` and rebuild ffmpeg with `--enable-libfdk-aac`.

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## For contributors & AI agents

This is a personal tool with a public repo. Contributions welcome but not expected.

- **Stack**: Rust (ffmpeg-next, mp4ameta) + TypeScript + Tauri 2 + Svelte
- **Architecture**: See `docs/external-apis/` for boundary docs
- **Agent guide**: `AGENTS.md` defines coding standards and workflows
- **Quality gates**: `scripts/checks.sh standard` before PRs
- **Optional hook auto-sync**: `git config core.hooksPath .githooks` to auto-sync/stage generated Tauri bindings during pre-commit when Rust IPC contract files are staged
- **Release flow**: use `.agents/skills/release-changelog/SKILL.md` as the canonical entrypoint, with `bun run release:notes -- --version <x.y.z> --date YYYY-MM-DD` then `bun run release:run -- --version <x.y.z> --changelog-verified --no-commit-tag|--commit-tag`

## Development

```bash
scripts/checks.sh standard    # Full quality gate
bun run tauri dev             # Dev mode
bun run test                  # All tests
```

**[Full technical reference →](docs/specs/technical-reference.md)** — architecture, data flows, IPC contracts, coding standards.
