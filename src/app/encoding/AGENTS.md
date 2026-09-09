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

- Request-shaped truth. VBR quality is not the sticky ABR/CBR/CVBR `bitrateKbps`.
  Estimate kbps uses the VBR table; it never parses `Est: ~60 kbps`.
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
- Per-encoder sample-rate restrictions come from `encoderConfigurations`.
  Unsupported explicit rates remain visible with a correction hint and are
  rejected at backend ingress; switching encoders must not silently resample.
- Afterburner is encoding truth. The checkbox stays in the Settings dialog.
- Two live App Runtimes isolate bags, capability loads, persist closures, and
  hints. Disposing A cannot publish into B.
- Estimated-size bytes stay in Output; this owner supplies kbps and channels.
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
