# Feature map

User-facing paths for live GUI drive. Not every panel. Composition shells
(`leftColumn`, `metadataManager`, `encodingWorkbench`) are layout, not
features.

## Baseline preconditions

- Doctor has run in this session. `live_drive` is `possible` before any
  live `drive` or `launch`.
- The instance was started by
  `.agents/skills/verify-audiobook-boss/verify-abb launch`, not by opening
  `/Applications/AudioBook Boss.app`.
- No real Audible session (Keychain
  `audiobook-boss.remote-source` / `audible.us.auth` absent).
- Fixtures are synthesized in the helper scratch directory. No personal
  library paths. No committed media.
- Output, if the feature writes files, is a scratch directory this run
  created. Do not use a saved user output folder from `app-settings.json`.
- Linux may run doctor, `features`, `scaffold`, and `drive --dry-run`
  only.

## Driving conventions

- Harness: macOS Accessibility via osascript against the window titled
  `AudioBook Boss`.
- Handle order: `aria-label`, visible name, `id`, `data-testid`.
- Native Tauri dialogs are OS sheets. Drive those sheets. Do not hunt for
  them in the Solid DOM.
- Do not click `Import from Library` on a machine with a real Audible
  account.
- Do not treat `http://localhost:1420` in a browser as this app.

## Proof and skip reporting

For each driven feature, record in `evidence/`:

- pass: action, handle, resulting state, side effect
- fail: expected state, actual state, evidence path
- skip: why (Linux live-drive block, isolate refuse, missing fixture
  tool). A skip is not a pass.

`--dry-run` must list the launch, AX, and dialog steps it did not run.

## Feature entry contract

Each feature file has:

1. One H1 and one paragraph of user-visible behavior
2. Exactly four H2s, in order: `Sub-features`, `How to get to it (user POV)`,
   `Driving it with osascript AX`, `Gotchas`
3. Sub-feature IDs, one line each
4. Driving section starting with `Preconditions:` then labeled bullets
   that pair a user action with a command and an observable result

## Features

| ID | File | User path |
| --- | --- | --- |
| `file-import` | `file-import.md` | Add local audio into Input and File Order |
| `file-list` | `file-list.md` | Select, order, and clear the audio list |
| `metadata` | `metadata.md` | Edit tags, cover, and online lookup |
| `encode-output` | `encode-output.md` | Choose encoder/output and start processing |
