## Public API Strip
- Import encoder request configuration from `src/ui/encoderPanel`.
- Exports: `applyEncodingDefaults`, `readEncoderDefaultsFromState`,
  `readEncodingRequestConfig`.

## Private Cluster
- Files: `EncoderPanelIsland.svelte`, `autoResolutionHints.ts`, `logic.ts`, `state.svelte.ts`, `toolchainValidationWorkflow.ts`, `toolchainValidationWorkflowLive.ts`, `toolchainValidationWorkflowServices.ts`, `__tests__/`.
- The cluster owns audio encoder UI state, resolved encoder availability,
  toolchain override state, sample-rate/channel state, and encoding request
  configuration truth. Selectable validity facts for encoder options, bitrate
  modes, bitrates, VBR levels, threads, sample rates, and channels come from the
  backend Runtime Settings Capabilities contract.

## Allowed Agent Edits Without Escalation
- Change internals when focused encoder panel tests stay green; run
  `mise run test:frontend` before handoff and `mise run runtime:contract`
  when public-strip or runtime contract surfaces change.
- Keep process-boundary encoding config reads behind `readEncodingRequestConfig`.
- Keep App Settings hydration/persistence coordination behind
  `applyEncodingDefaults` and `readEncoderDefaultsFromState`; do not let other
  panels reach into `state.svelte.ts`.
- Keep estimated-size updates derived from encoder state; do not mirror encoder/toolchain/sample-rate state into output or status panels.
- Keep UI labels and hints frontend-owned; do not reintroduce local
  accept/reject matrices for settings that Rust validates.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Moving encoder/toolchain/sample-rate request truth out of this cluster.
- Letting another panel import `state.svelte.ts`, `logic.ts`, or workflow internals to build process payloads.
