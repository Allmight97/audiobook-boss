## Public API Strip

- Import app settings hydration and persistence helpers from `src/ui/appSettings`.
- Exports: `hydrateAppSettings`, `initializeAppSettingsControlPlane`,
  `persistAppSettingsPatch`, `persistConcurrencyPreference`,
  `persistEncoderDefaults`, `persistOutputDefaults`.

## Private Cluster

- Files: `hydration.ts`, `persistence.ts`, `appSettings.test.ts`, `AGENTS.md`.
- The cluster coordinates durable preference hydration/persistence through
  `tauriClient` while leaving runtime request ownership in the job controls,
  encoder panel, and output panel Public API Strips.

## Allowed Agent Edits Without Escalation

- Add or adjust settings coordination when focused frontend and runtime boundary
  tests stay green.
- Persist backend/request-shaped settings only.
- Keep UI-only display state, detected availability text, previews, and panel
  visibility flags out of App Settings.

## Breaking-Change Triggers

- Bypassing `tauriClient` for settings commands.
- Importing private panel internals from outside this cluster's hydration
  coordinator instead of using the owning panel Public API Strips.
- Making App Settings apply runtime behavior directly instead of asking the
  owning runtime/control module to accept the change first.
