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

# First run publishes the AAXClean sidecar (.NET 10 SDK required)
# then starts Tauri with bundled FFmpeg and reusable logs.
bun run app:dev:log
```

Requires: macOS (Apple Silicon) or x86_64 Ubuntu 22.04 or newer (glibc 2.27+, OpenSSL, WebKitGTK 4.1), Bun 1.4 or newer, Rust (rustup), and a .NET 10 SDK for the sidecar. macOS also needs Homebrew. App, test, and release builds use **bundled FFmpeg** — Homebrew `ffmpeg` is not required to run the app. Install it only for the real-media test lane (fixture/readback). After clone, `bash scripts/setup.sh` installs everything except rustup and Homebrew.

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

After clone, `bash scripts/setup.sh` then `bash scripts/verify.sh`. Setup
prints the PATH line and never edits shell profiles. Lanes, owner mapping, and
focused loops: root `AGENTS.md` Environment and `scripts/AGENTS.md`.

Linux (tested target): x86_64 Ubuntu 22.04 or newer, glibc 2.27+, OpenSSL, and
WebKitGTK 4.1. `linux-arm64` publishes but is not proven on real hardware.
Known gap: saved sign-ins do not survive a reboot on Linux (WSL included).
`linux-keyutils-keyring-store` 1.0.0 is UntilReboot
([#557](https://github.com/Allmight97/audiobook-boss/issues/557)).

### Windows (WSL)

Windows is supported only through WSL running the Linux build.

- Use WSLg: Windows 11, or Windows 10 build 19044+ with the Microsoft Store WSL.
  Install the Linux requirements inside the WSL distro.
- If the window opens blank, launch with `WEBKIT_DISABLE_DMABUF_RENDERER=1`,
  and fall back to `WEBKIT_DISABLE_COMPOSITING_MODE=1`. That is a known
  WebKitGTK and WSLg issue. These remain launch notes until someone confirms
  them on a real WSL machine.
- Keep the library, `target/`, and outputs on the Linux filesystem (`~/…`),
  not `/mnt/c`. Windows drives are slower over WSL, chmod mostly does not
  apply, and they are case-insensitive.
- Dragging files from Windows Explorer into the app does not work under WSLg.
  Use the file picker.

### Install a local build

Build the current branch and replace `/Applications/AudioBook Boss.app` in
place. macOS (Apple Silicon) only; the replace is silent and unprompted.

```bash
bun run app:install-local            # local build, verify, install, prune artifacts
bun run app:install-local:existing   # install an already-built bundle (--skip-build)
```

On a supported Apple Silicon Mac, `bundled-ffmpeg` always uses the portable
Apple Silicon baseline (`ffmpeg-sys-next/build-portable`). `bun run app:build`
builds the repo-local app; `bun run app:build:dmg` and `bun run app:build:all`
package a DMG that may run on an unknown recipient Mac.

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
- Engine without a window: `bash scripts/abb-dev.sh <file-or-folder>...
  [--set field=value] [--save] [--out folder --export] [--json]` imports files
  into an engine session and can edit and save tags, choose audio and naming,
  export or preview with progress, cancel a title, and read back the exported
  tags. `--help` lists every option. It keeps its own state and never touches
  the app's settings.
- IPC/boundary checks: `bun run bindings:check:local` and
  `bun run bindings:check:runtime-boundary`. Use `bun run bindings:check` when
  release-critical drift confidence is required.
- Dependency hygiene: `bun run audit`.
- CI (`.github/workflows/ci.yml`): runs when a pull request opens ready or is
  marked ready, when auto-merge is enabled (the merge attempt), by hand, and
  twice a week on `main`. Pushes and drafts start nothing. It calls `scripts/setup.sh` and
  `scripts/verify.sh` for the frontend, core crates, engine, media, host, and
  Apple AAC on macOS, and Audible decrypt on both OSes. A pull request runs
  only the jobs its changes touch.
  GitHub also runs Pages for `site/**`.
- Bun is the package manager, script runner, and test runner.
- IPC bindings: `bun run bindings:generate`, `bun run bindings:check`, `bun run bindings:sync`
- Build timing: use direct Cargo timing commands such as `cargo build --timings`
  when investigating compile cost.
- Release lanes: use `.agents/skills/release`.
  `bun scripts/bump-version.ts <version>` updates version surfaces;
  `bun run app:install-local` is the developer-install lane and silently
  replaces `/Applications/AudioBook Boss.app`; `bun run app:build` builds a
  repo-local `.app`; `bun run app:build:dmg` builds a portable,
  noninteractive public DMG and rebuilds the AAXClean helper from current source.
  `bun scripts/resolve-release-dmg.ts --version <version>` resolves the artifact;
  download the uploaded asset and compare its `shasum -a 256` with the local DMG.

## Claude on GitHub

Tag `@claude` with a request in an issue or pull request comment, an inline
review comment, a submitted review, or a new issue's title/body. The
`Claude mentions` workflow handles the request. The triggering person must
have write access; the action rejects bot actors and users without it.

Both Claude workflows use a subscription token stored as the repository secret
`CLAUDE_CODE_OAUTH_TOKEN`. To replace a rejected or expired token, run these
commands locally, then paste the token printed by the first command into the
second command's hidden prompt:

```sh
claude setup-token
gh secret set CLAUDE_CODE_OAUTH_TOKEN --repo Allmight97/audiobook-boss
```

Paste only the token, without quotes or the surrounding instructions. Keep it
out of comments, logs, and committed files. A `401 Invalid bearer token` error
means Anthropic rejected the stored credential; rerunning with the same secret
does not repair it.

In GitHub Actions, select **Claude mentions → Run workflow** to check
authentication without posting comments or changing files. The run must pass
before testing `@claude` on a real request. From the CLI:

```sh
gh workflow run claude.yml --repo Allmight97/audiobook-boss --ref main
```

Automatic review is separate. Select **claude review → … → Enable workflow**
or **Disable workflow**, or use:

```sh
gh workflow enable claude-review.yml --repo Allmight97/audiobook-boss
gh workflow disable claude-review.yml --repo Allmight97/audiobook-boss
```

Toggling automatic review leaves mention handling available. Enabling review
does not retroactively review existing ready PRs; its next trigger is a PR
opening, reopening, or being marked ready. The workflow files must be on the
default branch before comment triggers and the manual authentication check
are available.

## Project Operation

- Agents: start in [AGENTS.md](AGENTS.md) and follow the nearest nested
  `AGENTS.md`; agent operating guidance lives there, not in this README.
