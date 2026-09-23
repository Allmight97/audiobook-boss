# Output Plan

## Scope

- Owns output directory, naming, path preview, export-size estimate, and
  collision review under `src/app/outputPlan/`.
- Solid views live in `src/ui/outputPanel` and `src/ui/collisionDialog`. They
  render this owner; they do not keep a second plan store.

## Public API Strip

- Import Output Plan runtime symbols from `src/app/outputPlan`.
  `createOutputOwner` is the Solid plan factory.
- Workbench callers that only need the composed UI strip import
  `src/ui/outputPanel` instead.
- `index.ts` is the export surface. Do not import `owner.ts`,
  `workflow.ts`, `collision.ts`, `estimate.ts`, or `previewDraft.ts` from outside this owner.

## Hard Invariants

- Explicit encoding estimates use each title's total source duration and
  Encoding's `estimateTitleKbps`; a null value means unknown size. Auto reports
  that size awaits audio planning rather than guessing copy compatibility.
  Explicit Preserve estimates sum source sizes; they do not use encoder targets.
  `estimate.ts` owns the byte formula; bitrate is total across channels.
- Path preview passes the selected title format to Rust. Audio owns its extension;
  changing quality or encoder availability alone does not re-run naming preview.
- Path preview is a Solid async memo on public Input, Metadata, output
  directory, naming preset, year, and the **committed** template. Live template
  typing updates the input immediately and commits after 150 ms. Do not preview
  on every keystroke. Preview retriggers when Metadata series or subseries part
  changes, not only title, album, or artist. Native path authority,
  metadata-intent validation, and request-id stale suppression stay in the
  preview workflow. Preview validation forwards `MetadataDraftValidation`
  through the injected `onMetadataValidation` dep. Do not add a Metadata setter
  here.
- Collision review is a separate preflight/review workflow
  (`runOutputPlanReviewWorkflow`) whose view and pending choice live on the
  Output owner. Views use `useAppRuntime().output`. Do not fold review into
  path-preview freshness.
- App Settings hydration passes resolved `outputDefaults` to the runtime's
  Output owner without persisting. User changes hand accepted defaults to the
  injected Settings `rememberOutputDefaults` intent.
- Processing submit and collision review use the injected Output owner
  (`readRequestConfig`, `openCollisionReview`). `readRequestConfig()` uses the
  live naming box (`namingTemplate`), not the 150 ms committed
  `previewTemplate`. Preview stays on the committed copy. Do not restore
  `updateOutputPath` or `updateEstimatedSize`, or a last-writer owner lookup.

## Testing

- `estimate.test.ts` pins the byte formula; `outputPlan.test.ts` covers its displayed estimate, unknown-size state and empty-session placeholder.
- `outputPlan.test.ts` pins hydration, derived estimate (including FDK VBR
  quality vs sticky request `bitrateKbps`), live submit naming vs 150 ms
  preview debounce, series-part preview retrigger, preview draft/source-path
  projection, and collision resolve/cancel. Duration comes from
  `runtime.input.replaceSession`.
- `workflow.test.ts` pins stale preview suppression and review approve /
  cancel / hard-block.
- `runtime-api-contract.test.ts` pins this owner's public export strip.
- Two-runtime request and collision isolation lives in
  `src/app/runtime/runtime.test.ts`.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Reading private Input, Metadata, file-list, or encoder state to build
  preview, estimate, or submit config.
- Moving the estimate formula or the Output header estimate span.
