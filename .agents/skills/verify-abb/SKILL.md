---
name: verify-abb
description: Prove an ABB change works by driving the real engine (abb-dev) and, when the UI changed, the real app, then keep ffprobe/JSON/screenshot evidence. Use before claiming a change works, when asked to verify, test end-to-end, or "check it actually works". Never touches the owner's library or stored credentials.
---

# Verify ABB

Unit tests and `verify.sh` lanes show that code agrees with its tests. This
skill shows that the product does the thing: real files go in, and real M4B
files come out with the right tags, chapters, and codec. A feature's script
exit code is its verdict. `verify.sh media` runs the same scripts, so CI
proves what you prove here.

## Launch

- Engine surface (default, headless): `bash .agents/skills/verify-abb/run.sh`.
  It builds `abb-dev` through `scripts/abb-dev.sh`, which runs cargo with
  `--locked` and the `bundled-ffmpeg` feature, then runs every feature. Name
  features to run fewer, for example `run.sh export chapters`.
- App surface (only when `src/**` or `src-tauri/**` changed): `bun run tauri dev`
  through `scripts/tauri.ts`. On a Linux box with no display, run it under
  `Xvfb :99` with `DISPLAY=:99`. On macOS, use the normal window.
- Ready signal: the engine is ready when `abb-dev --help` exits 0. The app is
  ready when the dev log prints the Vite URL and the window exists
  (`xdotool search --name audiobook` under Xvfb).

## Doctor

1. `bash scripts/setup.sh --check`. If it fails, run the one command it
   prints, then check again. Do not go on until it is clean.
2. `ffmpeg -version` must report the FFmpeg 9 CLI. Fixture synthesis needs it
   (`lib.sh` `tone`).
3. If `--check` mentions FAAC, `faac-sys`, or the bundled FFmpeg, read
   `vendor/faac-sys/AGENTS.md`. Never fall back to a system FFmpeg for the
   engine. A different engine is not a proof.

## Drive

- Engine: every call goes through `lib.sh` `abb_dev <state-name> ...`, which
  gives each call its own `--state-dir` under a temp `STATE`. Never pass the
  app's real state folder.
- Read results from `--json` output and ffprobe, never from abb-dev's exit
  code alone (`lib.sh` `format_tag`, plus ffprobe stream queries).
- App: click through the changed view in the Xvfb or macOS window. Save a
  screenshot before and after the action into the run's evidence folder.
  `app-use-notes` records what the owner sees; this step records what you
  saw.

## Evidence

- `.logs/verify/<UTC-run-id>/`: `<feature>.log`, the abb-dev JSON for each
  feature, `summary.txt` (`PASS`/`FAIL` lines), and screenshots when the app
  was driven.
- In the PR body, paste the `summary.txt` lines and name the evidence folder.
  A proof claim with no evidence folder is not a proof.
- The `verify.sh` receipt (`.logs/verification/receipts/`) records that the
  `media` lane passed for these inputs. The feature evidence records what the
  output looked like. Report both.

## Cleanup

- `run.sh` deletes `STATE` and `INPUTS` on exit (trap). Evidence stays.
- Stop the dev app (`bun run tauri dev`, and Xvfb if you started it). Leave
  no `abb-dev` processes: `pgrep -f abb-dev` should print nothing.
- Each git worktree has its own `target/`. Remove finished worktrees.

## Helpers

- `run.sh`: runs features in order, keeps going after a failure, and exits
  nonzero if any failed.
- `lib.sh`: `tone` (one-second sine fixture), `abb_dev` (isolated state),
  `format_tag` (ffprobe tag read), `fail`.
- `features/<name>.sh`: one feature per file. See the map below.

## Feature map

| Feature | Sub-features | How to get to it | Driving it | Gotchas |
|---|---|---|---|---|
| import | file → title with tags | `run.sh import` | `abb_dev import --json <file>`, then check the artist tag in the JSON | Inputs come from `tone` in `INPUTS`, never the owner's library. Folder import is not covered yet (gap) |
| metadata | edit, then save; tag read back | `run.sh metadata` | `--set genre=Mystery --save --json` | Check the tag with `format_tag`, not the JSON echo |
| export | M4B encode, AAC stream, template naming | `run.sh export` | `--export --format m4b --intent encode --bitrate 64 --template '{title}' --out` | Output name comes from the template; a wrong template fails on the missing file |
| collisions | default stops on an existing output; `rename` writes beside it | `run.sh collisions` | export twice to the same `--out`, then again with `--on-collision rename` | The second default export must fail. That failure is the pass |
| cancel | cancel one title; only the other is published | `run.sh cancel` | `--export --cancel-title 1`, then count published files (expect 1) | A timing-sensitive case; macOS AAC paths differ (see #554 and #575 CI failures) |
| chapters | chapter titles survive export | `run.sh chapters` | export an MP3 source, then ffprobe `chapter_tags=title` | Expects exactly "Opening Closing" |
| decrypt | AAX/AAXC via helper | `bash scripts/verify.sh decrypt` | not this skill | Synthetic fixtures; a wrong AAXC key is not the negative case |
| Audible download | real account | none | **blocked**: report BLOCKED | This skill never reads stored credentials |
| app UI | changed view or strip | Launch → App surface | Xvfb/macOS window, screenshots | Linux Xvfb shows layout; it does not prove macOS rendering |

A new feature is a new `features/<name>.sh` plus an entry in `run.sh`'s list
and a row here. A row with no script is a gap. Say so; do not paper over it.
