# Output Plan

## Scope

- Owns output directory, naming, path preview, encoded-size estimate, and
  collision review under `src/app/outputPlan/`.
- Solid views live in `src/ui/outputPanel` and `src/ui/collisionDialog`. They
  render this owner; they do not keep a second plan store.

## Public API Strip

- Import Output Plan runtime symbols from `src/app/outputPlan`.
  `createOutputOwner` is the Solid plan factory.
- Workbench callers that only need the composed UI strip import
  `src/ui/outputPanel` instead.
- `index.ts` is the export surface. Do not import `owner.ts`, `bind.ts`,
  `workflow.ts`, `collision.ts`, or `previewDraft.ts` from outside this owner.
- `applyOutputDefaultsFromSettings`, `readOutputDefaultsFromState`, and
  `readOutputRequestConfig` still resolve the last bound owner. They are
  compatibility exports: do not add callers. New callers use the Output owner
  held by App Runtime.

## Hard Invariants

- Estimated size is a derived view of public Input duration
  (`input.view().totalDurationSeconds`), encoder `encodingRequest` channels,
  and `encodingEstimateKbps`. Do not read private Input/encoder state, sample
  `readEncodingRequestConfig()`, parse the encoder `Est: ~60 kbps` label, or
  cache a mirrored byte size.
- Keep `estimateEncodedSizeBytes` as: non-positive duration → 0; bytes =
  `durationSeconds * bitrateKbps * 1000 / 8`; ×1.5 when `channels === 'stereo'`;
  ×1.03 overhead;   `Math.round`. On FDK VBR, `bitrateKbps` is the injected
  `encodingEstimateKbps`, not the sticky request `encoderSettings.bitrateKbps`.
  The encoder header is the only estimate consumer and keeps
  `~ 12.3 MB` / `~ --- MB` (`formatEstimatedSizeText`).
- Path preview is a Solid `createEffect` on public Input, Metadata, output
  directory, naming preset, year, and the **committed** template. Live template
  typing updates the input immediately and commits after 150 ms. Do not preview
  on every keystroke. Preview retriggers when Metadata series or subseries part
  changes, not only title, album, or artist. Native path authority is
  `computeOutputPathPreview` (plain async over Tauri `previewOutputPath`).
  Stale in-flight answers are suppressed by a request-id on this owner, not
  Effect. Metadata-intent validation is a separate apply path that must not
  block the path preview or write through it. Preview validation forwards
  `MetadataDraftValidation` through the injected `onMetadataValidation` dep.
  Do not add a Metadata setter here.
- Collision review is a separate preflight/review workflow
  (`runOutputPlanReviewWorkflow`) whose view and pending choice live on the
  Output owner. Views use `useAppRuntime().output`. Do not fold review into
  path-preview freshness.
- App Settings hydration passes resolved `outputDefaults` to the runtime's
  Output owner.
- Submit composition reads the runtime's Output owner. The current
  `readOutputRequestConfig()` / `readProcessingRequestConfig()` compatibility
  getters use the live naming box
  (`namingTemplate`), not the 150 ms committed `previewTemplate`. Preview stays
  on the committed copy. They are getters, not poke APIs. Do not restore
  `updateOutputPath` or `updateEstimatedSize`, or preserve the last-bound lookup
  through a new alias.

## Testing

- `estimate.test.ts` pins the byte formula and empty-session placeholder.
- `outputPlan.test.ts` pins hydration, derived estimate (including FDK VBR
  quality vs sticky request `bitrateKbps`), live submit naming vs 150 ms
  preview debounce, series-part preview retrigger, overlapping in-flight
  preview freshness, validation that does not block path preview, preview
  draft/source-path projection, and collision resolve/cancel. Duration comes
  from `runtime.input.replaceSession`.
- `workflow.test.ts` pins `computeOutputPathPreview` outcomes and review
  approve / cancel / hard-block.

## Breaking-Change Triggers

- Adding, removing, or renaming a public export.
- Reading private Input, Metadata, file-list, or encoder state to build
  preview, estimate, or submit config.
- Moving the estimate formula or the encoder-header estimate span.
