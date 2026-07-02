# Metadata Session

## Scope

- Applies to per-file metadata cache truth, pending draft/intent staging,
  draft validation adaptation, and the metadata batch-save workflow under
  `src/ui/metadataSession/`.
- This owner is the single frontend seam for creating and draining pending
  metadata truth. Every other strip stages through it and reads from it.

## Public API Strip

- Import metadata session runtime symbols from `src/ui/metadataSession`; do
  not reach into private files.
- Authoritative runtime export surface = `index.ts`, pinned by
  `__tests__/runtime-api-contract.test.ts`. Treat that test as the source of
  truth instead of a hand-listed export set here.
- The staging seam is outcome-shaped: callers hand `stageMetadataIntentPatch`
  a patch and branch on `'staged' | 'unchanged' | 'noop'`; merge, equality,
  and pending-set mechanics stay private.

## Private Cluster

- Files: `state.ts` (cache/intent/pending maps + outcome calls), `draft.ts`
  (draft-field filter and intent building), `validation.ts` (boundary
  validator adaptation), `saveState.ts` (save-in-progress store),
  `saveWorkflow.ts` (Effect workflow owner + `saveMetadataFromUI`),
  `__tests__/`.

## Does Not Own

The over-deepening guard — this owner must not absorb:

- Metadata form UI state (`src/ui/metadataForm`).
- Metadata lookup search/provider UI (`src/ui/metadataLookup`).
- Cover-art file/URL loading UI (`src/ui/coverArt`).
- Output preview rendering (`src/ui/outputPanel`).
- Status Panel rendering or processing launch (`src/ui/statusPanel`).
- Work Center operation truth (WorkRuntime renders save lifecycle).

## Hard Invariants

- Pending markers are **created** only through `stageMetadataIntentPatch`.
  Successful saves clear them through the save workflow; explicit
  file/session lifecycle clears (`removeMetadataForFile`,
  `clearMetadataSession`) may also remove pending state. No other writer.
- `cacheMetadataForFile` carries no pending semantics; it records backend
  read truth only.
- Intent merge, draft equality, and the pending-path set stay private; no
  caller reimplements the get→apply→compare→set ritual.
- Cover-only cache entries are not usable metadata
  (`isUsableMetadataCache`); they must not short-circuit full reads.
- The save workflow is the only writer of `metadataSaveInProgress`; UI
  surfaces get the readonly store.
- Artifact fields (`album_sort`, `comment`, `track`, `disk`) never enter
  `METADATA_DRAFT_FIELDS` (`draft.ts`, private): normal form staging cannot
  touch them; explicit clears flow through `stageMetadataIntentPatch` as
  `clear` ops (pinned by the contract test).
- Live save-workflow services capture cross-owner imports lazily (call-time
  arrows): this owner sits on static import cycles through `fileList` and
  `statusPanel`, and a value capture mid-cycle freezes `undefined` into the
  live layer.

## Testing State

The owner's proof set:

- `__tests__/runtime-api-contract.test.ts` — pins the export strip, the
  staging outcomes, and artifact-field preservation.
- `src/ui/__tests__/metadata-session-smoke.test.ts` — UI Workflow Smoke Test:
  edit→save (metadata-only) and lookup-apply→save through REAL session state
  with only Tauri/rendering boundaries mocked. This is the agent-verifiable
  end-to-end metadata-handling outcome; keep it green and unmocked at the
  session layer.
- Private-cluster suites in `__tests__/`: state cache/pending mechanics,
  draft filtering, validation reshaping, the fake-layer save-workflow
  harness, and the app-level save pending flow.
- Save-while-encoding is owned elsewhere: statusPanel's
  `processing-metadata-staging` test (payload level) and the backend
  media-execution lane (artifact truth).

## Breaking-Change Triggers

- Adding, removing, or renaming any Public API Strip export.
- Changing staging outcome semantics (`staged`/`unchanged`/`noop`), pending
  clearing behavior, or the validation outcome shape.
- Letting another owner import `state.ts`, `saveWorkflow.ts`, or other
  private files directly.
