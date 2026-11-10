# AudioBook Boss™

**Personal audiobook management for power users.**

Convert, tag, and organize your audiobook library with metadata that works everywhere — Audiobookshelf, Plex, and Apple Books.

## What it does

- **Batch convert** MP3/M4A/M4B/AAC/WAV/FLAC/MKA to optimized M4B audiobooks
- **Title stacks** — Group ordered source files into one tagged audiobook with chapters, alongside independent titles.
- **Audible acquire** — sign in, browse the library, and decrypt AAX/AAXC titles with the bundled FFmpeg
- **Audio choices** — Default to AAC/M4B, keep original audio when the selected output supports it, or choose Opus in M4A/MKA. Edit one title or a selected group; set defaults for future imports in Settings. Tags, artwork, chapters, and library folders still update when audio passes through. Opus direct playback and chapter navigation depend on the library and player.
- **Smart metadata** — series, narrator, cover art with Audiobookshelf/Apple Books dual-write compatibility
- **Parallel processing** with real-time progress and per-job cancellation
- **Metadata lookup** — search online databases and apply results in batch
- **Drag & drop** workflow — import files, edit tags, process, done

## Quick start

```bash
# Installs everything except rustup and Homebrew.
bash scripts/setup.sh

# Starts Tauri with bundled FFmpeg and reusable logs.
bun run app:dev:log
```

Requires: macOS (Apple Silicon) or x86_64 Ubuntu 22.04 or newer (OpenSSL, WebKitGTK 4.1), and Rust (rustup). macOS also needs Homebrew. Setup installs the Bun version `bun.lock` names. App, test, and release builds use **bundled FFmpeg** — Homebrew `ffmpeg` is not required to run the app. Install it only for the real-media test lane (fixture/readback).

**AAC runtime contract**: output encoder and input decoder are separate. Auto
selects Native NMR; Apple AAC and bundled FAAC are explicit choices.
Native AAC uses the bundled NMR coder with a numeric target and speed control.
Apple AAC uses a numeric target. Bundled FAAC offers Auto, AAC-LC, and HE-AAC v1
profiles with ABR or VBR. FAAC defaults to profile Auto and ABR; the shared
target defaults to 65 kbps, and sample rate/channels retain their Auto behavior.
FAAC chooses Auto’s profile from the requested output settings when encoding
opens: HE-AAC v1 when the bitrate per channel suits HE at the output rate (for
example 64 kbps stereo at 32, 44.1, or 48 kHz), otherwise AAC-LC. VBR offers Smaller (50), Standard (100), and Higher (200); size varies
with the audio, so use ABR for a bitrate target. Explicit HE supports 32, 44.1,
and 48 kHz; Auto and LC also support the other available output rates. Saved
HE/ABR preferences retain that intent.

Settings → Decoding chooses the AAC input decoder independently of the output
encoder. Auto selects a compatible FFmpeg decoder per file, including Apple's
`aac_at` on macOS. FAAD3 selects the bundled decoder for AAC-LC, HE-AAC v1/v2,
and ADTS with up to eight channels, which the output setting then downmixes;
other AAC profiles, including xHE-AAC, keep Auto. The choice is saved between
launches. `vendor/faac-sys/ABB-PROVENANCE.md` names the bundled FAAC and FAAD3 source. A failed FAAD3 decode is
reported without switching decoders.

All processing runs in the in-process Audio engine. Bundled source revisions
and wrapper changes are recorded under `vendor/`. ABB-produced FAAC HE files
retain Apple-compatible gapless timing; re-import accounts for SBR decoder
delay under both decoders. Third-party HE files declaring iTunSMPB also retain
their audio alignment and tail. FAAC and FAAD's LGPL license and source
provenance ship with the app; their source and build configuration live in
`vendor/faac-sys/`. Each public release provides the corresponding ABB source,
including the selected codec source and build scripts. To rebuild with a
modified FAAC or FAAD library, extract the matching
release source archive, edit `vendor/faac-sys/upstream/`, run
`bun install --frozen-lockfile`, then `bun run app:build:dmg` on an Apple Silicon
Mac with the prerequisites above. The build compiles and links that local codec
source; no proprietary relinking tool is required.

[Download latest release →](https://github.com/Allmight97/audiobook-boss/releases)

## Unattended work

Settings → Power enables **Keep computer awake while working** by default. It
prevents macOS idle sleep during encoding, Audible acquisition, Indexer handoff,
and metadata saves, while allowing display sleep. Turning it off takes effect
during active work. Leaving ABB open and idle does not prevent sleep; manual sleep, lid
closure, and low-battery sleep remain controlled by macOS.

## Development

After clone, `bash scripts/setup.sh` then `bash scripts/verify.sh`. Setup
prints the line that puts its tools on PATH, installs this repo's pre-commit
hook, and never edits shell profiles. Lanes and owner mapping: root
`AGENTS.md` Environment and `scripts/AGENTS.md`. Codex cloud: paste
`bash scripts/setup.sh` into the Codex environment settings field once; that
setting is not in the repo.

Linux is a tested target.
A saved Audible sign-in and indexer key go to the desktop secret service when
one is running, so they survive a reboot. With no secret service, they last
until the next restart, and the account view says sign-in will not be
remembered
([#557](https://github.com/Allmight97/audiobook-boss/issues/557)).
Windows has no native build. The plan is to run ABB under WSL, which
[#557](https://github.com/Allmight97/audiobook-boss/issues/557) has yet to prove.

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

`package.json` holds the shortcuts.

- Run the app: `bun run app:dev:log` (bundled FFmpeg).
- Read the app log: `~/Library/Logs/com.audiobook-boss/abb.log` on macOS,
  `~/.local/share/com.audiobook-boss/logs/abb.log` on Linux. A dev build
  (`bun run app:dev:log`) writes the same file name under its own
  `com.audiobook-boss.dev.<checkout id>` folder. A launch adds to it. When it is
  over 5 MB at launch, it becomes `abb.previous.log` and a new one starts.
- Run the checks: `bash scripts/verify.sh`; lanes are in `scripts/AGENTS.md`.
- Drive the engine without a window: `bash scripts/abb-dev.sh --help`. It keeps
  its own state and never touches the app's settings.
- Audit dependencies: `bun run audit`.

## Claude on GitHub

Tokens, workflows, and review toggles: [docs/claude-on-github.md](docs/claude-on-github.md).
