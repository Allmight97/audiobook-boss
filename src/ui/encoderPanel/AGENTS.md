# Encoder Panel

## Public API Strip
- Import encoder request configuration from `src/ui/encoderPanel`.
- Exports: `applyEncodingDefaults`, `encodingEstimateBitrateKbpsAtom`,
  `encodingRequestConfigAtom`, `readEncoderDefaultsFromState`,
  `readEncodingRequestConfig`, `readFdkAfterburner`, `setFdkAfterburner`.
- `readFdkAfterburner`/`setFdkAfterburner` exist for the App Settings dialog,
  which owns the afterburner control; the panel keeps the request-truth
  carrier and persistence rails and renders no afterburner UI.
- `applyEncodingDefaults(defaults, capabilities)` accepts an already-loaded
  Runtime Settings Capabilities encoder slice from App Settings hydration. It
  lazily imports `logic.ts` so the index stays side-effect-light for config
  consumers; keep new strip entries in that shape.
- `encodingRequestConfigAtom` is the reactive request-configuration strip
  Output Plan reads for channels and CBR/CVBR `bitrateKbps`. VBR quality does
  not change that request field. `encodingEstimateBitrateKbpsAtom` is the
  reactive numeric bitrate behind the `Est: ~60 kbps` line. Output Plan reads
  that number for the header size. Do not parse the Est: label. Do not sample
  `readEncodingRequestConfig()` or keep a mirrored size.

## Private Cluster
- Files: `EncoderView.tsx`, `encoderView.css`, `autoResolutionHints.ts`,
  `logic.ts`, `state.ts`, `view.ts`.
- `EncoderView.tsx` is the mounted Solid renderer and reads `state.ts`
  through a revision-counter plus `subscribeEncoderPanel`.
- The `estimated-size` span in `EncoderView.tsx` is the only consumer of
  Output Plan's `estimatedSizeTextAtom`. Keep `~ 12.3 MB` / `~ --- MB` in this
  header; do not move the span into the Output block.
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
  panels reach into `state.ts`.
- Keep estimated-size display derived from Output Plan; do not mirror encoder
  request config into an Output-owned size cache.
- Keep UI labels and hints frontend-owned; do not reintroduce local
  accept/reject matrices for settings that Rust validates.

## Breaking-Change Triggers
- Adding, removing, or renaming a Public API Strip export.
- Moving encoder/sample-rate request truth out of this cluster.
- Letting another panel import `state.ts`, `logic.ts`, or other private internals to build process payloads.
