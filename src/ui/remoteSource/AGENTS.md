# Remote Source UI

## Public API Strip

- `index.ts` exports `RemoteSourceAcquireView`; File Import composes it.
- Nonvisual consumers use the App Runtime Remote Source owner from
  `src/app/remoteSource`. Session assets and acquisition lifecycle stay there.

## Private Cluster

`RemoteSourceAcquireView.tsx` and `remoteSourceAcquire.css` own dialog rendering
and event wiring. The Source control switches Audible library and Indexer
search/Grab in one dialog. User edits and selections dispatch owner intents;
the view does not hydrate account state or construct selection-state patches.

Cover-preview scheduling follows the visible titles and is cancelled when the
view no longer needs it. Resources and caches remain private to the owner.
Indexer results are a list with separate selection buttons, per-row Grab, and
source-page links. Row Grab sits immediately left of View details and acts on
that row independently of selection. Command/Ctrl-click toggles individual
selections; plain click replaces selection. The toolbar says Grab All for
multiple selections and includes selections hidden by filtering; show the
hidden count. Opening details preserves selection and the current search.
Acquisition progress renders only in the Audible lane.

Remote source IPC routes through `src/lib/tauri/client.ts`. Materialized audio
imports through Input Session; processing remains user-triggered.
