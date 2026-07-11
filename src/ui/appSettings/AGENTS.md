# App Settings UI

## Public API Strip

- Import app settings hydration, persistence, and dialog entrypoints from
  `src/ui/appSettings`.
- Exports: `hydrateAppSettings`, `initializeAppSettingsControlPlane`,
  `persistAppSettingsPatch`, `persistConcurrencyPreference`,
  `persistEncoderDefaults`, `persistOutputDefaults`,
  `AppSettingsDialogIsland`, `openAppSettingsDialog`,
  `closeAppSettingsDialog`.

## Private Cluster

- Files: `hydration.ts`, `persistence.ts`, `settingsDialog.svelte.ts`,
  `AppSettingsDialogIsland.svelte`, `appSettings.test.ts`,
  `settingsDialog.test.ts`, `AGENTS.md`.
- The cluster coordinates durable preference hydration/persistence through
  `tauriClient` while leaving runtime request ownership in the job controls,
  encoder panel, and output panel Public API Strips.
- The settings dialog (Cmd+, via `App.svelte`) owns the user FFmpeg/FDK path
  preference, the FDK afterburner toggle (via the encoder panel's
  `readFdkAfterburner`/`setFdkAfterburner` strip), startup-behavior toggle,
  pin-current-as-defaults capture, and reset-all.

## Startup / Pinned-Defaults Semantics

- The panels auto-persist every change into the top-level (last-used) settings
  values; there is no separate session state. Capture ("Use current settings
  as defaults") is therefore a pure settings copy: top-level values →
  `pinnedDefaults`. Do not read panel internals to capture.
- `hydrateOnce` restores from `pinnedDefaults` only when
  `startupBehavior === 'pinnedDefaults'` AND a pin exists; otherwise it
  restores top-level values (today's remember-last behavior). Hydration must
  never persist — the panel appliers do not write, only the user-action
  handlers do.

## Allowed Agent Edits Without Escalation

- Add or adjust settings coordination when focused frontend and runtime boundary
  tests stay green.
- Persist backend/request-shaped settings only.
- Keep UI-only display state, detected availability text, previews, and panel
  visibility flags out of App Settings. **Approved exception:** the global
  `density` preference is durable user intent, like `startupBehavior`; it
  belongs to the top-level settings value and never to `pinnedDefaults`.

## Breaking-Change Triggers

- Bypassing `tauriClient` for settings commands.
- Importing private panel internals from outside this cluster's hydration
  coordinator instead of using the owning panel Public API Strips.
- Making App Settings apply runtime behavior directly instead of asking the
  owning runtime/control module to accept the change first.
- Changing startup-source selection or capture semantics above without updating
  the hydration and dialog tests that pin them.
