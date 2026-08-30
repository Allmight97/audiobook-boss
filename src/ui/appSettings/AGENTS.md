# App Settings UI

## Public API Strip

- Import app settings persistence from `src/ui/appSettings`.
- Exports: `persistAppSettingsPatch`, `persistConcurrencyPreference`,
  `persistEncoderDefaults`, `persistOutputDefaults`.
- Dialog state, hydration, and settings IPC moved to `src/app/appSettings`;
  import those from that owner, not from here.

## Private Cluster

- Files: `persistence.ts`, `AppSettingsDialogView.tsx`,
  `AppSettingsDialogView.test.tsx`, `appSettingsDialog.css`, `AGENTS.md`.
- `persistence.ts` is the durable write path through `tauriClient`; the encoder
  and output panels call it to persist their own defaults.
- `AppSettingsDialogView.tsx` reads Settings `dialog` through
  `useAppRuntime()` and holds no settings truth. Cmd+, is wired in
  `src/ui/App.tsx` through `settings.openDialog`.
- The dialog owns the user FFmpeg/FDK path preference, the FDK afterburner
  toggle (via the encoder panel's `readFdkAfterburner`/`setFdkAfterburner`
  strip), startup-behavior toggle, pin-current-as-defaults capture, and
  reset-all.

## Startup / Pinned-Defaults Semantics

- The panels auto-persist every change into the top-level (last-used) settings
  values; there is no separate session state. Capture ("Use current settings
  as defaults") is therefore a pure settings copy: top-level values →
  `pinnedDefaults`. Do not read panel internals to capture.
- Startup source selection is owned by
  `src/app/appSettings/startupDefaults.ts` and pinned by its sibling test.
  Read it there rather than re-deriving the rule at a call site.
- Hydration must never persist — the panel appliers do not write, only the
  user-action handlers do.

## Allowed Agent Edits Without Escalation

- Add or adjust settings coordination when focused frontend and runtime boundary
  tests stay green.
- Persist backend/request-shaped settings only.
- Keep UI-only display state, detected availability text, previews, and panel
  visibility flags out of App Settings.

## Breaking-Change Triggers

- Bypassing `tauriClient` for settings commands.
- Importing private panel internals instead of using the owning panel Public
  API Strips.
- Making App Settings apply runtime behavior directly instead of asking the
  owning runtime/control module to accept the change first.
- Changing startup-source selection or capture semantics without updating
  `src/app/appSettings/startupDefaults.test.ts` and the dialog tests.
