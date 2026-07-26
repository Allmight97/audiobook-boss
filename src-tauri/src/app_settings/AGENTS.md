# App Settings Directives

## Public API Strip

- Import durable settings behavior from `crate::app_settings`, not private child
  modules.
- Types: `AppSettings`, `AppSettingsPatch`, `EncoderDefaults`,
  `OutputDefaults`, `ConcurrencyPreference`, `ToolchainPreferences`,
  `StartupBehavior`, `DensityPreference`, `PinnedDefaults`.
- Functions: `get_app_settings`, `update_app_settings`, `reset_app_settings`.

## Private Cluster

- Files: `types.rs`, `storage.rs`, `contract_tests.rs`.
- The cluster owns durable preference schema, defaults, patch merge,
  validation, and private JSON storage under Tauri's app config directory.
- Durable preferences validate against the owning runtime APIs; App Settings
  must not duplicate encoder or JobRegistry accept/reject rules.

## Allowed Agent Edits Without Escalation

- Change storage or merge internals when App Settings contract tests and runtime
  binding checks stay green.
- Store backend/request-shaped settings only; keep UI-only display state out of
  durable settings. **Approved exceptions:** `DensityPreference` and
  `rail_width` are global, user-selected layout preferences persisted here
  alongside `StartupBehavior`; they are not panel visibility or transient
  display state. `rail_width` is clamped here (`RAIL_WIDTH_MIN/MAX`) so a
  stale or hand-edited file can never hydrate an unusable layout.
- Treat persisted user paths as preference data only. Runtime owners still
  validate paths before reads or writes.

## Breaking-Change Triggers

- Adding, removing, or renaming any Public API Strip symbol.
- Moving runtime behavior ownership, output artifact truth, audio toolchain
  validation, or Status Panel state into App Settings.
- Introducing a frontend persistence plugin dependency or bypassing
  `tauriClient` for settings commands.
