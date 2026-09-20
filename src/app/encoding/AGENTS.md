# Encoding Configuration

## Scope

- Owns encoder/sample-rate/channel request truth, capability projection,
  auto-resolution hints, and estimate kbps under `src/app/encoding/`.
- The Solid workbench lives in `src/ui/encoderPanel`. It renders this owner
  and dispatches `select`; it does not keep a second encoder store.

## Public API Strip

- Import `createEncodingOwner` and owner types from `src/app/encoding`.
- `index.ts` is the export surface. `owner.ts`, `project.ts`, `hints.ts`, and
  `estimate.ts` are private.
- Output and Processing take the runtime Encoding owner (`request` /
  `estimateKbps`). Do not add a bind slot or UI-global getter.

## Hard Invariants

- FDK quality, FAAC profile/rate/quality, numeric target kbps, and NMR speed
  stay independent across session encoder switches. Target bitrate includes
  all channels. FAAC ABR uses that target; FAAC VBR uses its capability presets
  and returns `null` from `estimateKbps` because size depends on the audio.
- Derive rate mode from the effective encoder's capability except FAAC, whose
  ABR/VBR choice is explicit. `encoderConfigurations` owns mode and rate
  support; its `faacProfiles` entries own profile-specific rates. Send profile
  Auto to FAAC through the request; the frontend does not reproduce upstream
  profile thresholds. Sample-rate/channel Auto still follows the input hints.
  While discovery is pending, preserve the validated hydrated mode so saved
  explicit encoder requests stay usable.
  `request()` rejects Auto without capabilities; estimates and saved defaults
  remain readable. Processing surfaces the rejection before preparation or
  submission. Build typed requests directly from this owner's state.
- Preserve a globally valid explicit sample rate when it is unsupported by the
  selected encoder, including selection, hydration, and capability reload.
  Show the unsupported-rate hint and disabled option; the user chooses the
  replacement and backend preflight rejects the incompatible request. Do not
  silently substitute Auto or a different explicit rate.
- Backend capabilities own numeric bounds. Apply bounds when hydrating defaults
  or reloading capabilities as well as accepting user edits.
- `applyDefaults` and capability clamp / unavailable-flavor snap to `auto`
  do not persist. Only `select` and `setAfterburner` persist last-used
  defaults through the injected Settings `rememberEncoderDefaults` intent.
- Capability and availability facts come from backend Runtime Settings
  Capabilities. `reloadCapabilities` accepts a fetched result from Settings
  and invalidates older loads; opening, rechecking, saving, and resetting
  share one scan across both owners after the configured path changes. Labels and
  auto-hints are frontend-owned.
- Selecting unavailable FDK invokes the injected setup intent and preserves the
  current encoder request. The FDK option remains actionable; capability loss
  during hydration/reload still clamps an unavailable request to Auto.
- Afterburner is encoding truth. The checkbox stays in the Settings dialog.
- Two live App Runtimes isolate bags, capability loads, persist closures, and
  hints. Disposing A cannot publish into B.
- Estimated-size bytes stay in Output; this owner supplies total kbps or an
  explicit unknown value for FAAC VBR.
  The `~ 12.3 MB` span stays in EncoderView.

## Testing

- Owner tests drive `view` / `request` / `estimateKbps` / `select` /
  `applyDefaults` with injected capability and persist adapters.
- Two-runtime proof lives in `src/app/runtime/runtime.test.ts`.
- EncoderView tests render through App Runtime; they do not import private
  encoder state.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reading encoder truth from `src/ui/encoderPanel` or a process-wide
  capability cache.
