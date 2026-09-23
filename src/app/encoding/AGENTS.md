# Encoding Configuration

## Scope

- Owns encoder/sample-rate/channel request truth, capability projection,
  source hints, and estimate kbps under `src/app/encoding/`.
- The Solid workbench lives in `src/ui/encoderPanel`. It renders this owner
  and dispatches `select`; it does not keep a second encoder store.

## Public API Strip

- Import `createEncodingOwner` and owner types from `src/app/encoding`.
- `index.ts` is the export surface. `owner.ts`, `project.ts`, `hints.ts`, and
  `estimate.ts` are private.
- Output and Processing take the runtime Encoding owner (`audioRequest` /
  `estimateTitleKbps`). Do not add a bind slot or UI-global getter.

## Hard Invariants

- FDK quality, FAAC profile/rate/quality, numeric target kbps, and NMR speed
  stay independent across session encoder switches. Target bitrate includes
  all channels. FAAC ABR uses that target; FAAC VBR uses its capability presets. Quality-based FDK and FAAC VBR
  return `null` from `estimateTitleKbps` because size depends on the audio.
- Derive rate mode from the effective encoder's capability except FAAC, whose
  ABR/VBR choice is explicit. `encoderConfigurations` owns mode and rate
  support; its `faacProfiles` entries own profile-specific rates. Send profile
  Auto to FAAC through the request; the frontend does not reproduce upstream
  profile thresholds. Sample-rate/channel Auto still follows the input hints.
  While discovery is pending, preserve the validated hydrated mode so saved
  explicit encoder requests stay usable.
  Build typed title requests directly from this owner; backend planning resolves
  availability only when encoding is required.
- Preserve a globally valid explicit sample rate when it is unsupported by the
  selected encoder, including selection, hydration, and capability reload.
  Show the unsupported-rate hint and disabled option; the user chooses the
  replacement and backend preflight rejects the incompatible request. Do not
  silently substitute Auto or a different explicit rate.
- Backend capabilities own numeric bounds. Apply bounds when hydrating defaults
  or reloading capabilities as well as accepting user edits.
- `applyDefaults` and capability clamps do not persist. Explicit encoder choices
  survive capability loss; unavailable choices cannot be newly selected. Only Settings `select` edits persist
  defaults through the injected Settings `rememberEncoderDefaults` intent.
- Capability and availability facts come from backend Runtime Settings
  Capabilities. `reloadCapabilities` accepts a fetched result from Settings
  and invalidates older loads; opening, rechecking, saving, and resetting
  share one scan across both owners after the configured path changes. Labels and
  auto-hints are frontend-owned. `capabilityRevision` invalidates derived audio
  previews after each current reload settles (including failure) and on reset,
  even when capability values or title requests are unchanged.
- Selecting unavailable FDK invokes the injected setup intent and preserves the
  current encoder request. The FDK option remains actionable; capability loss
  during hydration/reload retains the explicit request for backend validation.
- Afterburner is encoding truth. The shared encoder view shows it only for FDK;
  a title edit changes that title's request without changing saved defaults.
- Two live App Runtimes isolate bags, capability loads, persist closures, and
  hints. Disposing A cannot publish into B.
- Estimated-size bytes stay in Output; this owner supplies total kbps or an
  explicit unknown value for quality-based VBR.
  Output Plan owns the per-title size estimate.

## Testing

- Owner tests drive `view` / `audioRequest` / `select` /
  `applyDefaults` with injected capability and persist adapters.
- Two-runtime proof lives in `src/app/runtime/runtime.test.ts`.
- EncoderView tests render through App Runtime; they do not import private
  encoder state.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reading encoder truth from `src/ui/encoderPanel` or a process-wide
  capability cache.

## Defaults And Title Choices

Settings edits the durable default format, intent and encoding configuration.
Input snapshots a complete `TitleAudioRequest` when a title enters the session.
Later defaults edits never modify loaded titles. `applyDefaultsToTitles` is the
explicit replacement action. `audioRequest(file)` owns request composition.

`selectionView` projects common values and mixed fields; `selectTitles` applies
only the edited field to each target title. Encoder/quality/rate/channel edits
select Encode even when the selected value already matched. Editing encoding
preferences in Settings also selects User Preference (Encode). Selecting
Recommended retains those preferences for a later switch back. Capability facts
are shared, while source hints are derived for the edited title or selection.

`hydrateDefaults` accepts startup values only before explicit defaults edits; capability discovery does not count as a user edit. All output formats and intents persist through Settings. MP3 execution requests
carry no encoder settings; durable defaults keep encoding choices for subsequent
format switches. Opus uses VBR target kbps and backend-provided input rates.
