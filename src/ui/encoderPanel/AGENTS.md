# Encoder Panel

## Public API Strip
- Import encoder request configuration from `src/ui/encoderPanel`.
- Exports: `applyEncodingDefaults`, `readEncoderDefaultsFromState`,
  `readEncodingRequestConfig`, `readFdkAfterburner`, `setFdkAfterburner`.
- `readFdkAfterburner`/`setFdkAfterburner` exist for the App Settings dialog,
  which owns the afterburner control; the panel keeps the request-truth
  carrier and persistence rails and renders no afterburner UI.
- `applyEncodingDefaults(defaults, capabilities)` accepts an already-loaded
  Runtime Settings Capabilities encoder slice from App Settings hydration.
- Composition-only UI shells may import `EncoderWorkbenchIsland.svelte`; do not
  use the main encoder index for renderer exports because it must stay
  side-effect-light for config consumers.

## Private Cluster
- Files: `EncoderWorkbenchIsland.svelte`, `autoResolutionHints.ts`, `logic.ts`,
  `state.svelte.ts`, `view.ts`, `__tests__/`.
- The cluster owns audio encoder UI state, resolved encoder availability,
  sample-rate/channel state, and encoding request configuration truth.
  Selectable validity facts for encoder options, bitrate modes, bitrates, VBR
  levels, sample rates, and channels come from the backend Runtime Settings
  Capabilities contract.

## Allowed Agent Edits Without Escalation
- Change internals when focused encoder panel tests stay green; run targeted
  Vitest files when proving UI behavior and generated-binding/Public API Strip/runtime
  contract checks when runtime surfaces change.
- Keep process-boundary encoding config reads behind `readEncodingRequestConfig`.
- Keep App Settings hydration/persistence coordination behind
  `applyEncodingDefaults` and `readEncoderDefaultsFromState`; do not let other
  panels reach into `state.svelte.ts`.
- Keep estimated-size updates derived from encoder state; do not mirror encoder/sample-rate state into output or status panels.
- Keep UI labels and hints frontend-owned; do not reintroduce local
  accept/reject matrices for settings that Rust validates.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Moving encoder/sample-rate request truth out of this cluster.
- Letting another panel import `state.svelte.ts`, `logic.ts`, or other private internals to build process payloads.
