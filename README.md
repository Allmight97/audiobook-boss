# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC/WAV/FLAC to optimized M4B audiobooks
- **Book Binder** — Merge multiple chapterized audio files into a single M4B with chapter markers and metadata.
- **Audible acquire** — sign in, browse the library, and materialize AAX/AAXC titles through the bundled helper
- **Smart metadata** — series, narrator, cover art with Audiobookshelf/Apple Books dual-write compatibility
- **Parallel processing** with real-time progress and per-job cancellation
- **Metadata lookup** — search online databases and apply results in batch
- **Drag & drop** workflow — import files, edit tags, process, done

## Quick start

```bash
# JS/TS dependencies (Bun 1.4.0; see package.json packageManager)
bun install

# First run publishes the AAXClean sidecar (.NET 8 SDK required)
# then starts Tauri with bundled FFmpeg and reusable logs.
bun run app:dev:log
```

Requires: macOS (Apple Silicon), Bun 1.4.0, Rust, and a .NET 8 SDK for the sidecar. App, test, and release builds use **bundled FFmpeg** — Homebrew `ffmpeg` is not required to run the app. Install it only for the real-media test lane (fixture/readback) or an optional external-FDK encoder.

**AAC runtime contract**: output encoder and input decoder are separate. Normal processing uses in-process `ffmpeg-next`. FDK HE-AAC output uses an external FFmpeg/`libfdk_aac` adapter and may force `aac_at` or `libfdk_aac` when the default decoder cannot handle the source.

[Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Toolchain

- Package manager: **Bun 1.4.0**, pinned by `package.json` `packageManager`.
- Verify Bun or frontend changes with the commands below; use `.agents/skills/release`
  for packaging and GitHub Release work.

## Development

Run targeted validation from the script guide for the owner or risk surface you
touched. There is no default broad review command.

### Install a local build

Build the current branch and replace `/Applications/AudioBook Boss.app` in
place. macOS (Apple Silicon) only; the replace is silent and unprompted.

```bash
bun run app:install-local            # native build, verify, install, prune artifacts
bun run app:install-local:existing   # install an already-built bundle (--skip-build)
```

On a supported Apple Silicon Mac, source app builds and developer installs
target the compiling host natively. `bun run app:build` builds that native
repo-local app. `bun run app:build:dmg` and `bun run app:build:all` instead use
the portable Apple Silicon baseline because their DMG may run on an unknown
recipient Mac.

## Script Guide

Human index for common commands. `package.json` owns shortcuts and
`scripts/AGENTS.md` owns the exact fresh-agent verification command menu. ABB
currently uses direct native commands, not a custom verification runner or a
default broad review route.

- Core dev: `bun run app:dev:log` (bundled FFmpeg). `bun run build` is the
  frontend production bundle only.
- Verification is owner-scoped. Run the smallest native command that proves the
  touched owner, then escalate only when the change crosses owners or a safety,
  data, or contract invariant requires it.
- Frontend checks: `bun run typecheck`,
  `bun run test -- <test files>`, plus `bun run fmt:check` / `bun run lint:check`
  when formatting or lint is in scope.
- Focused Rust loops:
  `cargo nextest run -p abb-audible-core`,
  `cargo nextest run -p abb-media-core`,
  `cargo nextest run -p abb-metadata-core`,
  `cargo nextest run -p abb-output-artifact-core`,
  `cargo nextest run -p abb-processing-core`,
  `cargo nextest run -p abb-remote-source-core`,
  `cargo nextest run -p audiobook-boss --features bundled-ffmpeg --lib`, or
  `cargo nextest run -p audiobook-boss --features bundled-ffmpeg --test all_tests`.
- IPC/boundary checks: `bun run bindings:check:local` and
  `bun run bindings:check:runtime-boundary`. Use `bun run bindings:check` when
  release-critical drift confidence is required.
- Dependency hygiene: `bun run audit`
  It is not part of the normal review path.
- CI: GitHub automatically runs Pages for `site/**` and a path-narrowed
  frontend clean-install alarm (frozen install, typecheck) after
  relevant `main` pushes. It is an alarm, not a PR gate. Rust tests and
  generated-binding checks stay local or release-owned; an empty PR check list
  does not mean those proofs ran on GitHub.
- Tooling policy: Bun is the package manager/script runner/test runner.
  Keep Vite scripts on the standard Vite CLI unless a validated tooling
  decision changes that.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- Build timing: use direct Cargo timing commands such as `cargo build --timings`
  when investigating compile cost.
- Release lanes: use `.agents/skills/release`.
  `bun scripts/bump-version.ts <version>` updates version surfaces;
  `bun run app:install-local` is the native developer-install lane and silently
  replaces `/Applications/AudioBook Boss.app`; `bun run app:build` builds a
  native repo-local `.app`; `bun run app:build:dmg` builds a portable,
  noninteractive public DMG and rebuilds the AAXClean helper from current source.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the artifact;
  `gh release verify-asset` proves the uploaded file matches that local DMG.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and then follow the nearest nested `AGENTS.md`.
- Grey-box module work is governed by the Public API Set in [docs/ubiquitous-language.md](docs/ubiquitous-language.md): Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, WorkRuntime, Status Panel Runtime, Audio Engine Deep Module, App Settings, and RemoteSourceRuntime.
- For substantial planning and alignment, start from the owner and task frame in `docs/system-map.md`; use global `grill-me` when action-changing forks remain. Default durable capture is GitHub issues (`docs/agents/issue-tracker.md`). Use `docs/specs/` only when explicitly requested. Session handoffs belong in OS temp, not the repo. External presentation artifacts belong under `/Users/jstar/Documents/Codex/artifacts/audiobook-boss`.
- For external library/API behavior, use `.agents/skills/abb-library-research` as the control plane for lockfile versions, installed or registry-packaged source, Context7, exact public package docs, and exceptional ephemeral upstream retrieval.
- For the product/system shape, state/control model, owner map, and source-of-truth
  ladder, use [docs/system-map.md](docs/system-map.md); use
  [docs/ubiquitous-language.md](docs/ubiquitous-language.md) for canonical terms.
- For a quick runtime boundary index, use [docs/api-map.md](docs/api-map.md), then verify in code before changing contracts or behavior.
- UI work is not done from static inspection alone. Use targeted tests for deterministic behavior and browser-agent or human review for visual/UX outcomes.
- Current behavior lives in code, manifests, generated contracts, and executed
  proof. `AGENTS.md` owns operating invariants, `docs/DECISIONS.md` owns durable
  rationale, and open GitHub issues own mutable plans—not current behavior.
  The conflict/load order is mapped in [docs/system-map.md](docs/system-map.md).
  `.artifacts/` is temporary local state only.
