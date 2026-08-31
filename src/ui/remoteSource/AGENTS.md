# Remote Source UI

## Public API Strip

- Import session-asset coordination from `src/ui/remoteSource`.
- Exports: `companionSummaryForInputIds`, `hasSupplementalAssetsForInputId`,
  `purgeRemoteSourceSessionsForInputIds`, `releaseRemoteSourceSessionRetainers`,
  `retainRemoteSourceSessionsForInputIds`, `registerRemoteSourceSupplementalAssets`,
  `subscribeRemoteSourceSupplementalAssets`,
  `supplementalAssetsByInputIdForProcessing`, `CompanionAssetSummary` (type).
- Do not re-export acquisition workflow symbols or private session-asset
  helpers such as `supplementalAssetsForInputIds` from the index.
- These non-view session-asset exports are current #471 compatibility rails.
  Do not add callers; after migration, nonvisual consumers import the App
  Runtime Remote Source owner and this UI index exposes the dialog view.
- The acquire dialog is `RemoteSourceAcquireView.tsx`. File Import composes
  it. Import acquire intents from `src/app/remoteSource`.

## Private Cluster

- Files: `RemoteSourceAcquireView.tsx`, `remoteSourceAcquire.css`.

Owner truth lives in `src/app/remoteSource`. This directory renders it. The
current session-asset re-exports for Processing, File List, and inspector
callers are removed by #471 rather than preserved as a second public seam.

Remote source IPC must route through `src/lib/tauri/client.ts`. Materialized
audio imports through Input Session; processing remains user-triggered.

## Shape

Solid owns rendering and event wiring. Effect owns account/acquisition
workflows. Pure display and selection policy belong in
`src/app/remoteSource` helpers.
