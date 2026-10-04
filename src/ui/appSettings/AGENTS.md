# App Settings UI

## Scope And Public Strip

- Export `AppSettingsDialogView` and `SettingsPersistenceNotice` from `index.ts`.
  They render `useAppRuntime().settings` and dispatch owner intents. Preference
  acceptance, persistence, capture, and reset follow
  `src/app/appSettings/AGENTS.md`.
- Cmd+, is wired in `src/ui/App.tsx` through `settings.openDialog`.
- Show the persistence notice in Settings when open, and in the workbench when
  closed. Render the owner's failure/retry state and accepted control values.

## View Interactions And Proof

- Settings owns its scrolling body; all sections and the bottom actions remain
  reachable. Opening Settings starts at Audio defaults. Audio defaults apply to future imports; existing
  titles change only through their explicit Apply App Settings action.

- Indexer connection fields dispatch Remote Source intents. Its API key input
  is write-only. HTTPS is the recommended URL example; explicit HTTP remains
  usable with visible transport guidance. Connection help is view-local, and
  Escape dismisses it before dismissing Settings.
- Reset requires the existing second activation. Reset controls remain
  disabled while a dialog save is pending.
- The engine always starts with settings in effect (an unreadable file
  starts from defaults), so the dialog has no load error; the confirmed full
  reset stays reachable.
- `AppSettingsDialogView.test.tsx` owns dialog interactions and visible automatic
  save failure/retry. `runtime-api-contract.test.ts` pins this UI export strip.
