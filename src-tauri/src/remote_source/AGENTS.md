# RemoteSourceRuntime

`remote_source` owns remote-source provider state, account/session lifecycle,
acquisition jobs, release search/grab (Indexer lane), staging roots, acquired
session files, Supplemental Assets, and cleanup.

## Public API Strip

Allowed external entrypoints:

- Provider-neutral command types re-exported from `mod.rs`.
- `RemoteSourceRuntime` methods called by `src-tauri/src/commands/remote_source.rs`.

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
- Auth providers separate the live OAuth exchange from keychain persistence
  (a `persist_auth`-style vault write seam in `providers/audible/mod.rs`) so
  the serialize -> `set_secret` -> `account_state` write path is mock-testable
  without a network `register()` call.
- `staging.rs` owns ABB-managed staging/session roots and cleanup rules.
- `scoped_output.rs` owns staged remote writes: `prepare` (stale-partial pre-clean)
  → partial write → cancel check → same-directory `rename_and_commit`. Drop cleans
  uncommitted paths. `ProvisionalCommittedFile` holds committed audiobook output
  until validation and supplemental steps succeed.
- Post-download cancel uses `rollback_committed_file`; do not revive generic
  `cleanup_download_artifacts` helpers. No cross-device rename fallback here.
- `providers/audible/library.rs` owns Audible library response shaping.
- `providers/indexer/` owns Indexer connection persistence, release
  search/grab, and the first Prowlarr HTTP adapter. Indexer grabs do not create
  acquisition jobs or materialize files into Input.
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

Materialized handoff files must remain usable after provider logout. Do not
purge a session containing materialized files unless the FileList/session-asset
owner has removed the corresponding imported inputs.

Audible Supplemental PDF acquisition uses provider-private authenticated
`GET /companion-file/{title_id}`. Do not use `HEAD`; Audible API `pdf_url`
fields are presence hints, not direct-download facts.

## Failure Truth

Remote providers may return typed unsupported/protected/auth statuses. They must
not fake acquisition success, silently fall back to manual import, or enqueue
placeholder files as materialized sources.
