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

## For contributors & AI agents

This is a personal tool with a public repo. Contributions welcome but not expected.

- **Stack**: Rust (ffmpeg-next, mp4ameta) + TypeScript + Tauri 2 + Svelte
- **Docs map**: start in `docs/README.md` for canonical docs routing
- **Architecture**: `docs/specs/technical-reference.md` is the current architecture/runtime map
- **Verification**: `docs/verification.md` defines proof-of-done by change type, including UI harness verification
- **Browser harness**: `docs/browser-harness.md` explains the split between required scenario verification and optional interactive browser review
- **Workloop**: `docs/workloop.md` defines the repo's local task-runner contract and keeps `.agent-work/` explicitly non-durable
- **Controlplane skill**: use `.agents/skills/controlplane-operator/SKILL.md` when a task is about operating ABB through the repo substrate rather than only changing product code
- **Agent guide**: `AGENTS.md` defines coding standards, workflows, and local policy routing
- **Quality gates**: `scripts/checks.sh standard` before PRs
- **Optional hook auto-sync**: `git config core.hooksPath .githooks` to auto-sync/stage generated Tauri bindings during pre-commit when Rust IPC contract files are staged
- **Release flow**: use `.agents/skills/release-changelog/SKILL.md` as the canonical entrypoint, with `bun run release:notes -- --version <x.y.z> --date YYYY-MM-DD` then `bun run release:run -- --version <x.y.z> --changelog-verified --no-commit-tag|--commit-tag`

## Repo Control Plane

Audiobook Boss now treats the repo itself as part of the delivery system, not just the app code.

- **Required UI proof**: `bun run harness:verify --changed` maps UI-affecting edits to real browser scenarios and emits local artifacts instead of relying on static inspection or memory.
- **Optional browser review**: `bun run harness:agent` starts a persistent Playwright-backed review session so agents can inspect desktop layout, controls, and visible behavior in a live loop.
- **Workloop execution**: `WORKFLOW.md` plus `bun run work:*` provide a repo-native single-task runner with isolated worktrees, temporary task branches, explicit cleanup, and no durable task archive.
- **Durable versus temporary truth**: code, canonical docs, and decisions are durable; `.agent-work/` and `.artifacts/` are local runtime evidence only.

The point of this substrate is straightforward: give agents and humans a tighter loop, clearer proof-of-done, and better guardrails against subtle UI/runtime mistakes that are easy to miss in a repo without an executable harness.

## Development

```bash
scripts/checks.sh standard    # Full quality gate
bun run tauri dev             # Dev mode
bun run test                  # All tests
bun run harness:verify --changed # Required UI proof for UI-affecting work
bun run harness:agent start --scenario metadata-edit # Optional live desktop browser-review loop
```

**[Docs map →](docs/README.md)** — canonical docs routing, verification guidance, architecture/runtime reference, and decision log.
