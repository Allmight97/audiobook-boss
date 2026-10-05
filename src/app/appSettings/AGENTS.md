# App Settings

## Ownership And Public Strip

- The engine owns the settings in effect, their validation, and whether they
  are saved (`crates/abb-engine/src/app_settings/AGENTS.md`). This owner shows
  the engine's `SettingsSnapshot`, holds the Settings dialog's state, and sends
  `SettingsIntent`s through `engineLink`.
- Import `createSettingsOwner` and owner types from `src/app/appSettings`;
  `owner.ts` is private.
- Views dispatch semantic Settings intents; they do not call settings IPC.
  Audio and output defaults are not set here: the session records them when
  they change, and announces the settings with a `settings-update` event.

## What Stays Here

- Wording and display: durability state from `save_error`, the concurrency
  view (Auto shows the capability's `autoEffective`, independently of the
  current fixed count), and dialog visibility and controls.
- UI-only disclosure, detected text, previews, and visibility stay outside
  durable settings.

## Proof

- `owner.test.ts`: what the dialog shows and which intents it sends, against
  the fake engine. Write ordering, retry, and reset rules are proved in the
  engine's `runtime_tests.rs`.
- `src/__tests__/public-api-strips.contract.test.ts` independently pins the owner export strip.
- Visible failure/retry and dialog interactions live in `src/ui/appSettings`.
