# Encoder Panel

## Scope

- `EncoderView.tsx` is the Solid encoder view and owns markup, interaction
  wiring, labels, and owner-local CSS.
- Encoder/sample-rate/channel request truth is still module-global in this
  directory. That is a current lifetime gap, not the target owner shape.

## Current Compatibility Strip

- Import the retained encoder strip from `src/ui/encoderPanel` only where
  current code still requires it.
- Exports: `applyEncodingDefaults`, `estimateKbpsFromRequest`,
  `readEncoderDefaultsFromState`, `readEncodingRequestConfig`,
  `readFdkAfterburner`, `setFdkAfterburner`, `subscribeEncoderPanel`.
- Do not add callers to `subscribeEncoderPanel`, `state.ts`,
  `src/ui/runtimeSettingsCapabilities.ts`, or the read/apply global functions. New
  callers use a runtime-scoped owner composed by App Runtime.

## Required Owner Boundary

- Runtime capabilities, current encoder/sample-rate/channel intent,
  request/default projections, automatic resolution hints, and estimate facts
  belong to one runtime-scoped frontend owner, not this view directory.
- Processing and Output receive that owner from App Runtime rather than reading
  this UI index.
- App Settings hands defaults to, and reads accepted defaults from, the owner.
  Automatic persistence/failure state belongs to App Settings, not this
  view.
- Two live runtimes must be able to hold different encoder requests without a
  shared listener, capability cache, hydration promise, or reset.

## Private Cluster

- Current files: `EncoderView.tsx`, `encoderView.css`,
  `autoResolutionHints.ts`, `logic.ts`, `requestConfig.ts`, `state.ts`,
  `view.ts`.
- The `estimated-size` span in `EncoderView.tsx` is the only consumer of Output
  Plan's estimated-size text. Keep `~ 12.3 MB` / `~ --- MB` in this header;
  do not move the span into Output.
- Selectable validity facts come from backend Runtime Settings Capabilities;
  the view does not reproduce accept/reject tables.

## Invariants

- Keep request-shaped encoder settings explicit. VBR quality is not the sticky
  CBR/CVBR `bitrateKbps` field, and estimate code must not parse the
  `Est: ~60 kbps` label.
- UI labels and hints stay frontend-owned; encoder availability and accepted
  setting facts stay backend-owned.
- Hydration applies values without persisting them. User intents may persist
  only through the App Settings owner contract.

## Done Criteria

- Current changes use focused Encoder view/config tests and runtime-boundary
  checks when request shapes change.
- A lifetime migration proves two live App Runtimes isolate requests and
  deletes compatibility globals/re-exports instead of keeping aliases.
