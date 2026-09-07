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

- Indexer connection fields dispatch Remote Source intents. Its API key input
  is write-only. HTTPS is the recommended URL example; explicit HTTP remains
  usable with visible transport guidance. Connection help is view-local, and
  Escape dismisses it before dismissing Settings.
- Reset requires the existing second activation. Disable conflicting dialog
  edits while reset is saving.
- `AppSettingsDialogView.test.tsx` owns dialog interactions and visible automatic
  save failure/retry. `runtime-api-contract.test.ts` pins this UI export strip.
