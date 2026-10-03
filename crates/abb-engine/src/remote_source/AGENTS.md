# RemoteSourceRuntime

`remote_source` owns remote-source provider state, account/session lifecycle,
acquisition jobs, release search/grab (Indexer lane), staging roots, acquired
session files, Supplemental Assets, and staging cleanup (the session decides
when an imported download goes).

## Public API Strip

Allowed external entrypoints:

- Provider-neutral command types re-exported from `mod.rs`.
- Host account display reads plus auth and logout through `Engine::remote_source()`.
- `RemoteUiIntent` and `RemoteUiSnapshot`, including their named connection,
  Indexer-work, and per-release status vocabulary, through the session boundary. Connection
  mutation, selection, Search/Grab, and acquisition start/cancel stay engine-internal.

Processing, audio, metadata, output artifact, and frontend code must not import
or infer provider-private Audible internals.

## Private Cluster

- `providers/audible/` owns Audible-specific protocol behavior and diagnostics.
- `providers/audible/http/` owns shared HTTP client, redirect policy, streaming,
  and cancellation checks for audio and supplemental PDF downloads.
- `providers/audible/acquisition/{mod,paths,progress,supplemental,validation}.rs`
  owns title acquisition orchestration; production validation stays in
  `validation.rs`.
- `providers/audible/library_probe.rs` owns library probe test harness helpers.
- `cancellation.rs` owns shared acquisition cancellation checks.
- `vault.rs` owns backend secret-vault adapters.
- Credential service names follow the app identifier so development identities
  cannot read, replace, or delete production provider credentials. The production
  identifier retains its shipped service name; isolated profiles require their
  own sign-in and keys.
- Auth providers separate the live OAuth exchange from keychain persistence
  (a `persist_auth`-style vault write seam in `providers/audible/mod.rs`) so
  the serialize -> `set_secret` -> `account_state` write path is mock-testable
  without a network `register()` call.
- `staging.rs` owns ABB-managed staging/session roots and cleanup rules.
- `scoped_output.rs` owns staged remote writes: `prepare` (stale-partial pre-clean)
  → partial write → cancel check → same-directory `rename_and_commit`. Drop cleans
  uncommitted paths. `ProvisionalCommittedFile` holds committed audiobook output
  until validation and supplemental steps succeed.
- Post-download cancel uses `rollback_committed_file`. No cross-device rename
  fallback here.
- `providers/audible/library.rs` owns Audible library response shaping.
- `providers/indexer/` owns Indexer connection persistence, release
  search/grab, and the first Prowlarr HTTP adapter. Indexer grabs do not create
  acquisition jobs or materialize files into Input. Grab logging correlates
  start, HTTP response, and outcome with a request ID; keep credentials,
  release GUIDs/URLs, and raw response bodies out of those entries.
- Release detail URLs are optional source-provided HTTP(S) links without embedded
  credentials; never infer them from a release GUID.
- Indexer connection URLs reject embedded credentials on save and draft
  testing. The URL and categories live in `indexer.toml` under App Settings'
  storage rules: written crash-safe, a failed write logged and reported; a
  file damaged outside ABB (including a URL with credentials) loads as not
  configured and is never sent to IPC or a provider. API keys stay in the
  credential store.
- The private connection owner resolves URL, categories, and the host's key for
  search/grab together; its credential-bearing result never crosses IPC.
- Indexer credentials are scoped to the normalized server URL in the vault;
  connection JSON never contains a key. Save persists changed JSON before
  changing that URL's key, and reports partial persistence if the vault fails.
  A failed save must never pair one server with another server's key.
- Connection Test accepts a draft without persisting it. An omitted draft key
  resolves only from that draft URL's vault slot; a new URL requires its own key.

No provider secrets, license blobs, raw provider responses, or protected
intermediates may cross the public strip or generated TypeScript boundary.

Supplemental Assets may cross only as provider-neutral, validated asset facts.
Processing receives them explicitly by file-list `inputId`; it must not query
`RemoteSourceRuntime`.

A finished job hands its files to the session through the engine-set
`Handoff` and records the outcome on the job. From then on the session
decides when the download goes, including one nothing was imported from
(`crate::session`, `staged.rs`), and calls `purge_session`.
Cancel does nothing to a job that already finished, so it cannot remove files
the session holds. Materialized handoff files stay usable after provider
logout. Disconnect refuses unfinished acquisition/handoff. Job changes update
`SessionUpdate.remote`; download progress is published at most every 100 ms,
a stage change at once. Snapshot `terminal` and `settled` facts belong here.

Audible Supplemental PDF acquisition uses provider-private authenticated
`GET /companion-file/{title_id}`. Do not use `HEAD`; Audible API `pdf_url`
fields are presence hints, not direct-download facts.

## Working Remote State

- `ui.rs` owns accepted title/PDF/release choices, normalized connection drafts,
  readback generations, and batch outcomes. Its intent match is routing; effect
  execution stays in `RemoteUiRun`. Accepted runs execute on `EngineTasks` through
  the session, regardless of whether a host awaits the reply.
- Available PDFs start included; refresh preserves explicit exclusion while
  pruning titles that cannot be acquired. A Grab batch captures its releases,
  sends sequentially, and keeps per-release failures for explicit retry. Its
  connection lease spans gaps between requests; Save and lane replacement are
  refused until it finishes, and a connection Save and a search refuse each
  other the same way. A release sent since the last search or Save is not sent
  again (the grab answers "Already sent").
- Connection readback cannot erase newer typing or a newer Save. Old Test/Search
  results cannot replace newer requests or changed lanes. API keys remain private;
  snapshots report only configured/entered facts. Initialize the HTTP client on
  first use so a session that never uses Indexer pays no network setup cost.
- Acquisition admission and disconnect share the remote UI guard. A late library
  reply or terminal publication cannot restore disconnected choices or jobs.
  One unsettled acquisition includes its pending Input handoff, not only download.
- Pure choice/readback tests and local HTTP sequential-batch proof live in
  `ui_tests.rs`; provider protocol tests remain with their provider owner.

## Failure Truth

Remote providers may return typed unsupported/protected/auth statuses. They must
not fake acquisition success, silently fall back to manual import, or enqueue
placeholder files as materialized sources. Dash/Widevine acquisition stays
unsupported until ABB has its own CDM, MPD/PSSH, and content-key design.
