# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC/WAV/FLAC to optimized M4B audiobooks
- **Title stacks** — Group ordered source files into one tagged audiobook with chapters, alongside independent titles.
- **Audible acquire** — sign in, browse the library, and materialize AAX/AAXC titles through the bundled helper
- **Audio choices** — Default to AAC/M4B, keep original audio when the selected output supports it, or choose Opus in M4A/MKA. Edit one title or a selected group; set defaults for future imports in Settings. Tags, artwork, chapters, and library folders still update when audio passes through. Opus direct playback and chapter navigation depend on the library and player.
- **Smart metadata** — series, narrator, cover art with Audiobookshelf/Apple Books dual-write compatibility
- **Parallel processing** with real-time progress and per-job cancellation
- **Metadata lookup** — search online databases and apply results in batch
- **Drag & drop** workflow — import files, edit tags, process, done

## Quick start

```bash
# JS/TS dependencies
bun install

# First run publishes the AAXClean sidecar (.NET 8 SDK required)
# then starts Tauri with bundled FFmpeg and reusable logs.
bun run app:dev:log
```

Requires: macOS (Apple Silicon), Bun 1.4 or newer, Rust, and a .NET 8 SDK for the sidecar. App, test, and release builds use **bundled FFmpeg** — Homebrew `ffmpeg` is not required to run the app. Install it only for the real-media test lane (fixture/readback).

**AAC runtime contract**: output encoder and input decoder are separate. Auto
selects Native NMR; Apple AAC and bundled FAAC are explicit choices.
Native AAC uses the bundled NMR coder with a numeric target and speed control.
Apple AAC uses a numeric target. Bundled FAAC offers Auto, AAC-LC, and HE-AAC v1
profiles with ABR or VBR. FAAC defaults to profile Auto and ABR; the shared
target defaults to 65 kbps, and sample rate/channels retain their Auto behavior.
FAAC chooses Auto’s profile from the requested output settings when encoding
opens. VBR offers Smaller (50), Standard (100), and Higher (200); size varies
with the audio, so use ABR for a bitrate target. Explicit HE supports 32, 44.1,
and 48 kHz; Auto and LC also support the other available output rates. Saved
HE/ABR preferences retain that intent. All processing runs in the in-process
Audio engine; on macOS it uses `aac_at` to decode AAC sources the default
decoder cannot handle. Bundled source revisions and wrapper changes are
recorded under `vendor/`. ABB-produced FAAC HE files retain Apple-compatible
gapless timing; ABB accounts for native decoder priming when reading them back.
FAAC's LGPL license and source provenance ship with the app; its corresponding
source and build configuration live in `vendor/faac-sys/`. Each public release
provides the corresponding ABB source, including the selected FAAC source and
build scripts. To rebuild with a modified FAAC library, extract the matching
release source archive, edit `vendor/faac-sys/upstream/`, run
`bun install --frozen-lockfile`, then `bun run app:build:dmg` on an Apple Silicon
Mac with the prerequisites above. The build compiles and links that local FAAC
source; no proprietary relinking tool is required.

[Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Unattended work

Settings → Power enables **Keep computer awake while working** by default. It
prevents macOS idle sleep during encoding, Audible acquisition, Indexer handoff,
and metadata saves, while allowing display sleep. Turning it off takes effect
during active work. Leaving ABB open and idle does not prevent sleep; manual sleep, lid
closure, and low-battery sleep remain controlled by macOS.

## Toolchain

- Package manager: **Bun**, a `devDependency` updated with the other packages;
  `bash scripts/locked-bun-version.sh` prints the version CI uses.
- Verify Bun or frontend changes with the commands below; use `.agents/skills/release`
  for packaging and GitHub Release work.

## Development

Run the checks in the script guide for what you changed.

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

Index of common commands; `package.json` holds the shortcuts.

- Core dev: `bun run app:dev:log` (bundled FFmpeg). `bun run build` is the
  frontend production bundle only.
- Frontend checks: `bun run typecheck`,
  `bun run test -- <test files>`, plus `bun run fmt:check` / `bun run lint:check`
  when formatting or lint is in scope.
- Focused Rust loops:
  `cargo test --locked -p abb-audible-core`,
  `cargo test --locked -p abb-media-core`,
  `cargo test --locked -p abb-metadata-core`,
  `cargo test --locked -p abb-output-artifact-core`,
  `cargo test --locked -p abb-processing-core`,
  `cargo test --locked -p abb-remote-source-core`,
  `cargo test --locked -p abb-engine --features bundled-ffmpeg --lib`,
  `cargo test --locked -p abb-engine --features bundled-ffmpeg --test all_tests`, or
  `cargo test --locked -p audiobook-boss --features bundled-ffmpeg` (Tauri host).
- Engine without a window: `cargo run -p abb-engine --features bundled-ffmpeg
  --bin abb-dev -- <file-or-folder>... [--set field=value] [--save]
  [--out folder --export] [--json]` imports files into an engine session and
  can edit and save tags, choose audio and naming, export or preview with
  progress, cancel a title, and read back the exported tags. `--help` lists
  every option. It keeps its own state and never touches the app's settings.
- IPC/boundary checks: `bun run bindings:check:local` and
  `bun run bindings:check:runtime-boundary`. Use `bun run bindings:check` when
  release-critical drift confidence is required.
- Dependency hygiene: `bun run audit`.
- CI (`.github/workflows/ci.yml`): runs when a pull request opens ready or is
  marked ready, when auto-merge is enabled, by hand, and twice a week on
  `main`. Pushes and drafts start nothing. It checks the frontend, the core
  crates, the engine with the real-media lane, the Tauri host, the generated
  bindings, and Apple AAC on macOS. A pull request runs only the jobs its
  changes touch. GitHub also runs Pages for `site/**`.
- Bun is the package manager, script runner, and test runner.
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
  download the uploaded asset and compare its `shasum -a 256` with the local DMG.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and follow the nearest nested
  `AGENTS.md`; agent operating guidance lives there, not in this README.
