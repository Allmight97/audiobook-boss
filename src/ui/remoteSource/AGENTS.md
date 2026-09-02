# Remote Source UI

## Public API Strip

- Export only `RemoteSourceAcquireView` from `src/ui/remoteSource`.
- File Import composes the acquire dialog through this index.
- Nonvisual consumers use the composed App Runtime Remote Source owner; do not
  re-export owner state, workflows, session assets, or cache controls here.

## Private Cluster

- Files: `RemoteSourceAcquireView.tsx`, `remoteSourceAcquire.css`.

Owner truth lives in `src/app/remoteSource`. This directory renders it.

Remote source IPC must route through `src/lib/tauri/client.ts`. Materialized
audio imports through Input Session; processing remains user-triggered.

## Shape

Solid owns rendering and event wiring. The app owner owns plain-async
account/acquisition workflows. Pure display and selection policy belong in
`src/app/remoteSource` helpers.
