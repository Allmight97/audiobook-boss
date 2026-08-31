# App Settings UI

## Scope

- `AppSettingsDialogView.tsx` renders App Settings from
  `useAppRuntime().settings`; it owns markup, interaction wiring, and CSS, not
  settings truth or persistence policy.
- Cmd+, is wired in `src/ui/App.tsx` through `settings.openDialog`.

## Current Compatibility Strip

- `src/ui/appSettings` currently exports `persistAppSettingsPatch`,
  `persistConcurrencyPreference`, `persistEncoderDefaults`, and
  `persistOutputDefaults` from `persistence.ts`.
- Those helpers swallow failure after `console.warn`. This is a confirmed #421
  gap. Do not add callers or treat silent automatic persistence as the target
  contract.

## Target Owner Boundary

- Settings hydration, accepted runtime values, automatic persistence,
  durability/failure state, dialog intent, startup behavior, pinned-defaults
  capture, and reset coordination live in `src/app/appSettings`.
- Views and sibling owners dispatch semantic Settings intents. They do not call
  `tauriClient.updateAppSettings` or import a UI persistence helper.
- #421 selects and proves the observable failure outcome. Default: retain an
  accepted runtime value, publish that it is not yet durable, and allow retry;
  restore only when the runtime owner also rejected the value.

## Startup / Pinned-Defaults Semantics

- Accepted panel changes persist into the top-level (last-used) settings
  values. Capture ("Use current settings as defaults") is a settings copy:
  top-level values → `pinnedDefaults`; it does not read panel internals.
- Startup source selection is owned by
  `src/app/appSettings/startupDefaults.ts` and pinned by its sibling test.
  Read it there rather than re-deriving the rule at a call site.
- Hydration must never persist — the panel appliers do not write, only the
  user-action handlers do.

## Invariants And Proof

- Persist backend/request-shaped settings only. UI-only disclosure, detected
  text, previews, and visibility stay outside App Settings.
- App Settings asks the owning runtime/control module to accept behavior before
  recording the preference.
- Startup-source or capture changes update
  `src/app/appSettings/startupDefaults.test.ts` and dialog/owner tests.
- Persistence failure changes prove visible owner state and retry/restore
  semantics through the App Settings Public API Strip.
