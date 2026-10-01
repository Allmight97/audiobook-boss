# App Settings

## Ownership And Public Strip

- The engine owns the settings in effect, their validation, and whether they
  are saved (`crates/abb-engine/src/app_settings/AGENTS.md`). This owner shows
  the engine's `SettingsSnapshot`, holds the Settings dialog's state, and sends
  `SettingsIntent`s through `engineLink`.
- Import `createSettingsOwner` and owner types from `src/app/appSettings`;
  `owner.ts` is private.
- Views and sibling owners dispatch semantic Settings intents. Runtime
  composition injects `rememberEncoderDefaults` and `rememberOutputDefaults`;
  views do not call settings IPC.

## What Stays Here

- Wording and display: durability state from `save_error`, the concurrency
  view (Auto shows the capability's `autoEffective`, independently of the
  current fixed count), and dialog visibility and controls.
- Startup: `loadStartupDefaults` returns the snapshot's startup defaults and
  rejects while settings are unreadable. App Runtime hands them to Encoding and
  Output Plan before import, so OS-opened and remote imports receive stored
  audio defaults. Hydrating a panel never writes settings.
- After a reset, `bindAfterReset` hands the new defaults to the panels.
- Audio edits in Settings record top-level defaults for future imports; title
  edits do not persist.
- UI-only disclosure, detected text, previews, and visibility stay outside
  durable settings.

## Proof

- `owner.test.ts`: what the dialog shows and which intents it sends, against
  the fake engine. Write ordering, retry, and reset rules are proved in the
  engine's `runtime_tests.rs`.
- `runtime-api-contract.test.ts` independently pins the owner export strip.
- Visible failure/retry and dialog interactions live in `src/ui/appSettings`.
