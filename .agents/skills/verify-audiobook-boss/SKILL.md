---
name: verify-audiobook-boss
description: Drive AudioBook Boss in the live Tauri desktop GUI when user-visible behavior must be proven in a real window. Use this instead of unit tests or audit-test-value when the acceptance surface is the desktop app a user would click.
---

# Verify AudioBook Boss

Primary surface is the Tauri 2 + Solid desktop window titled `AudioBook Boss`.
Vitest, Nextest, and `audit-test-value` stay on their own lanes. This skill
drives the live GUI.

Vite-only `bun run dev` on port 1420 is not the desktop app. `/lab.html` and
`/docs/design/ui-directions-v3.html` are not the app. Never treat a browser
tab on `:1420` as live Tauri drive.

## Launch

From the repo root, on macOS Apple Silicon only:

```bash
.agents/skills/verify-audiobook-boss/verify-abb launch
```

That wrapper is the lever. It starts a verification instance with
`bun run app:dev:log` (bundled FFmpeg, logs under `.logs/`). Do not run
`/Applications/AudioBook Boss.app`. Do not run `bun run app:install-local`.

Ready when all of these are true:

1. `.logs/tauri-dev.log` (or the run-scoped file it points at) contains
   `Starting AudioBook Boss application`.
2. A window exists with title `AudioBook Boss`.
3. The process command contains `target/debug/audiobook-boss`, not
   `/Applications/AudioBook Boss.app`.
4. Port 1420 is held by the Vite server this `app:dev:log` session started
   (`devUrl` in `src-tauri/tauri.conf.json`).

Teardown only the instance this run created:

```bash
.agents/skills/verify-audiobook-boss/verify-abb cleanup
```

Never `killall`, never `pkill audiobook-boss`, never kill by process name.
`scripts/dev-tauri-log.sh` will replace an existing ABB-owned listener on
port 1420. If 1420 is already taken, refuse and stop. Do not steal a user's
dev session.

## Doctor

```bash
.agents/skills/verify-audiobook-boss/verify-abb doctor
```

Read-only. Worth driving only when doctor prints `live_drive: possible` and
exits 0.

Doctor fails closed (nonzero) when any of these hold:

- The runner is not macOS Apple Silicon. Linux must not fake a window.
- The installed app is running, or the path to drive would be
  `/Applications/AudioBook Boss.app`.
- Port 1420 is in use by a process this helper did not start.
- A `target/debug/audiobook-boss` process is already running and was not
  started by this helper (shared identifier `com.audiobook-boss`, no
  profile override).
- A real Audible session is present (macOS Keychain service
  `audiobook-boss.remote-source`, account `audible.us.auth`).

The verification instance and the installed app share Tauri
`app_config_dir` (`app-settings.json`) and the same Keychain service.
There is no data-dir or profile switch in this repo. Two instances cannot
run side by side. If isolate checks fail, refuse. Do not attach to a
shared or user instance.

On Linux, doctor still prints a structured report, then exits nonzero with
`live_drive: blocked`. Help, doctor, feature list, fixture scaffold, and
`--dry-run` stay available.

## Drive

This repo has no Playwright, Cypress, tauri-driver, or debug IPC for GUI
drive. The harness is macOS Accessibility via osascript, aimed at the
window titled `AudioBook Boss`.

Prefer stable handles in this order: `aria-label`, visible name, `id`,
`data-testid`. Do not click by coordinates.

```bash
.agents/skills/verify-audiobook-boss/verify-abb drive file-import --dry-run
.agents/skills/verify-audiobook-boss/verify-abb drive file-import
```

`--dry-run` prints the user path and the exact osascript/AX steps, records
what it skipped (no Tauri launch, no window, no native dialog), and must
not start the app. Live `drive` without `--dry-run` is macOS Apple Silicon
only and must fail with that requirement on Linux.

Read `features/README.md` before the first drive. Drive one mapped feature
file at a time. Feature files name the handles and the observable result.

Native file and folder pickers are Tauri `plugin-dialog` sheets, not DOM.
After clicking `Add Folder` (`#add-folder-btn`) or `Add audio files`
(`aria-label`), drive the sheet with osascript to the scratch fixture
directory this helper created. Do not point the sheet at a personal library.

Never open `Import from Library` / `#acquire-audiobooks-btn` against a real
Audible account. Never type a real Amazon handoff URL.

## Evidence

Named location:

`.agents/skills/verify-audiobook-boss/evidence/`

`evidence/README.md` says what lands there. Cleanup does not delete it.

A pass is all of:

- The path a user would take (mapped feature), not a test-only shortcut.
- Each action paired with a resulting window, list, field, status, or file
  state.
- Side effects named (scratch output created, `.logs/tauri-dev.log` line,
  status text). Mocks are allowed only at a production boundary the
  feature file already names.
- `--dry-run` evidence shows the launch/AX/dialog steps that were skipped.

On a live session, also keep `.logs/tauri-dev-summary.md` and
`.logs/tauri-dev.log`. Those are latest-run entrypoints owned by
`scripts/dev-tauri-log.sh`, not this skill's evidence directory.

## Cleanup

```bash
.agents/skills/verify-audiobook-boss/verify-abb cleanup
```

Stops the verification instance this helper launched (recorded PIDs only).
Deletes the scratch fixture directory this helper created. Leaves
`.agents/skills/verify-audiobook-boss/evidence/` in place.

## Helpers

Run from the repo root. The executable is
`.agents/skills/verify-audiobook-boss/verify-abb`.

```bash
.agents/skills/verify-audiobook-boss/verify-abb help
.agents/skills/verify-audiobook-boss/verify-abb doctor
.agents/skills/verify-audiobook-boss/verify-abb features
.agents/skills/verify-audiobook-boss/verify-abb scaffold
.agents/skills/verify-audiobook-boss/verify-abb drive file-import --dry-run
.agents/skills/verify-audiobook-boss/verify-abb launch
.agents/skills/verify-audiobook-boss/verify-abb drive file-import
.agents/skills/verify-audiobook-boss/verify-abb cleanup
```

`launch` and live `drive` fail closed on Linux. `help`, `doctor`,
`features`, `scaffold`, `drive --dry-run`, and `cleanup` must work there.

## Isolate rules

- Never drive or attach to `/Applications/AudioBook Boss.app` or any
  user-installed binary.
- Never use a real Audible session or personal library.
- Synthesize fixtures at runtime in a temp dir. Do not commit media files.
- Live GUI drive needs macOS Apple Silicon. Linux doctor and live-drive
  subcommands fail closed. Do not fake a window.
- The verification instance and the installed app share identifier
  `com.audiobook-boss`. If an installed or foreign instance is already
  using that identity, port 1420, or the Keychain secret, refuse.

## Completion

Pass when:

1. Doctor ran and its `live_drive` field matches the host.
2. One mapped feature was driven in the Tauri window, or live GUI drive is
   recorded as blocked on macOS with doctor plus a dry-run transcript kept
   under `evidence/`.
3. Cleanup left evidence in place and removed only this run's instance and
   scratch.
4. No product code, tests, or other skills were edited.

Fail when doctor was skipped, a browser tab or Vite-only page was treated
as the desktop app, the installed app was driven, a real Audible session
was used, evidence is missing after cleanup, or a documented dry command
exits nonzero.

## Pointers

- `features/README.md` owns the feature map, baseline preconditions, and
  proof/skip reporting. Read it before opening a feature file.
- `features/*.md` own one user-facing path each: handles, drive commands,
  observable proof. Read the one feature you will drive.
- `scripts/dev-tauri-log.sh` owns `app:dev:log` capture and port-1420
  reclaim. Read it only when launch or teardown behavior is in doubt.
- `.agents/skills/audit-test-value` owns test-value audits. Do not use this
  skill for that.
- `.cursor/` is leftover Cursor plugin config (gitignored). It is not a
  skills tree. Durable owner is this directory.
