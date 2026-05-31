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
- Verify Bun changes with `bun scripts/proof/runner.ts review`;
  use `release` for packaging/release work.

## Development

```bash
bun install
bun run tauri dev
bun scripts/proof/runner.ts review
bun run test
```

## Script Guide

Human index for common commands. `package.json` owns shortcuts,
`bun scripts/proof/runner.ts --help` owns proof routes, and
`scripts/AGENTS.md` maps script internals.

- Core dev: `bun run tauri dev`, `bun run build`, `bun run test`
- Main quality gate: `bun scripts/proof/runner.ts review`
  Use `review quick` for static/boundary proof, `release` for packaging, and
  `focus ...` routes for owner-local proof. Successful proof runs discard temp
  logs; failed runs print the OS-temp evidence directory and summary path.
  `bun run proof:clean` removes legacy repo-local `.proof/` debris.
- Policy checks: `bun run check:fallback`, `bun run check:no-bridge`
  Allowed import/export surface drift is checked by `scripts/check-public-api-strips.sh`.
- Dependency hygiene: `bun run check:deps`
  Run explicitly, or use `bun scripts/proof/runner.ts diagnose deps`; it is not part of the normal review gate.
- Tooling policy: Bun is the package manager/script runner/test runner.
  Keep Vite scripts on the standard Vite CLI unless a proof-backed tooling
  decision changes that.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- xHE-AAC fixture proof: `ABB_XHE_AAC_FIXTURE=/path/to/book.m4b bun scripts/proof/runner.ts focus rust media-manual xhe-aac`
  Requires an auto-detectable FDK-capable external FFmpeg; the fixture is local-only and not committed.
- Build timing: `bun scripts/proof/runner.ts diagnose timing`
  Use timing proof for compile/build feedback; do not infer compile cost from late-stage spinner labels.
- Release: use `.agents/skills/release`.
  `scripts/bump-version.sh <version>` updates version surfaces;
  `bun run app:build` builds a repo-local `.app`; `bun run app:install-local`
  installs `/Applications/AudioBook Boss.app`; `bun run app:build:dmg` builds a DMG.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the release artifact before publishing a GitHub Release.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- Grey-box module work is governed by the seven Public APIs documented in [docs/ubiquitous-language.md](docs/ubiquitous-language.md): Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, Status Panel Runtime, Audio Engine Deep Module, and App Settings.
- For substantial planning, roadmap, architecture, or implementation alignment, use `.agents/skills/decision-alignment` to align outcomes with the repo owner. Keep active specs under `docs/specs/` only while the work needs a temporary repo-carried plan; delete them or distill enduring truths into canon when done. External presentation artifacts belong under `/Users/jstar/Documents/Codex/artifacts/audiobook-boss`.
- For external library/API behavior, use `.agents/skills/abb-library-research` as the control plane for authenticated Context7/current docs, squashed `repos/*` reference source, route cards, subtree refresh guidance, and installed dependency truth. Reference repos are read-only research material, not app dependencies.
- For the product/system shape, use [docs/system-map.md](docs/system-map.md) and [docs/ubiquitous-language.md](docs/ubiquitous-language.md).
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Durable truth lives in code, GitHub issues, this file, and [AGENTS.md](AGENTS.md). `.artifacts/` is temporary local state only.
