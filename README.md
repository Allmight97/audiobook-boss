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

**AAC runtime contract**: output encoder choice and input decoder choice are separate concerns. The app stays on the single `ffmpeg-next` engine, but may select compatible AAC decoders such as `aac_at` or `libfdk_aac` at runtime for AAC-family inputs that the default decoder cannot handle. For higher quality AAC encoding and broader AAC decode compatibility on macOS, `brew install fdk-aac` and rebuild ffmpeg with `--enable-libfdk-aac`.

Requires: macOS (Apple Silicon). [Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Development

```bash
bun install
bun run tauri dev
scripts/checks.sh standard
bun run test
bun run test:controlplane
bun run harness:verify --changed
bun run issue:run --issue 251
```

## Project Operation

- Agents: start in [AGENTS.md](/Users/jstar/Projects/audiobook-boss/AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- UI work is not done from static inspection alone. Run `bun run harness:verify --changed` for UI-affecting changes.
- Execution-ready GitHub issues can run end-to-end through `bun run issue:run --issue <number>`.
- Execution-ready issues must include `Goal`, `Constraints`, `Acceptance`, `Validation`, `Delivery Mode`, `Human Review`, and the marker `<!-- abb:issue-kind=ready -->`.
- The default runner flow is: read issue, create isolated worktree and branch, run local Codex with [WORKFLOW.md](/Users/jstar/Projects/audiobook-boss/WORKFLOW.md), validate, commit, open or reuse a PR when `Delivery Mode=pr`, and comment back on the issue.
- `Human Review=visual` means finish mechanical validation first, then hand off with an explicit visual-review note.
- Durable truth lives in code, GitHub issues and PRs, this file, [AGENTS.md](/Users/jstar/Projects/audiobook-boss/AGENTS.md), and [WORKFLOW.md](/Users/jstar/Projects/audiobook-boss/WORKFLOW.md). `.agent-work/`, local worktrees, and `.artifacts/` are temporary state only.
