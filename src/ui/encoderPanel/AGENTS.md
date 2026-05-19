## Public API Strip
- Import encoder request configuration from `src/ui/encoderPanel`.
- Exports: `readEncodingRequestConfig`.

## Private Cluster
- Files: `EncoderPanelIsland.svelte`, `autoResolutionHints.ts`, `logic.ts`, `state.svelte.ts`, `toolchainValidationWorkflow.ts`, `toolchainValidationWorkflowLive.ts`, `toolchainValidationWorkflowServices.ts`, `__tests__/`.
- The cluster owns audio encoder UI state, resolved encoder availability, toolchain override state, sample-rate/channel state, and encoding request configuration truth.

## Allowed Agent Edits Without Escalation
- Change internals when focused encoder panel tests and `scripts/check-public-api-strips.sh` stay green.
- Keep process-boundary encoding config reads behind `readEncodingRequestConfig`.
- Keep estimated-size updates derived from encoder state; do not mirror encoder/toolchain/sample-rate state into output or status panels.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Moving encoder/toolchain/sample-rate request truth out of this cluster.
- Letting another panel import `state.svelte.ts`, `logic.ts`, or workflow internals to build process payloads.
