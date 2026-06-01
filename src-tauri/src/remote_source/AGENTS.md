# RemoteSourceRuntime

`remote_source` owns remote-source provider state, account/session lifecycle,
acquisition jobs, staging roots, acquired session files, Supplemental Assets,
and cleanup.

## Public API Strip

Allowed external entrypoints:

- Provider-neutral command types re-exported from `mod.rs`.
- `RemoteSourceRuntime` methods called by `src-tauri/src/commands/remote_source.rs`.

Processing, audio, metadata, output artifact, and frontend code must not import
or infer provider-private Audible internals.

## Private Cluster

- `providers/audible/` owns Audible-specific protocol behavior and diagnostics.
- `vault.rs` owns backend secret-vault adapters.
- `staging.rs` owns ABB-managed staging/session roots and cleanup rules.
- `providers/audible/library.rs` owns Audible library response shaping.

No provider secrets, license blobs, raw provider responses, or protected
intermediates may cross the public strip or generated TypeScript boundary.

Supplemental Assets may cross only as provider-neutral, validated asset facts.
Processing receives them explicitly by file-list `inputId`; it must not query
`RemoteSourceRuntime`.

## Failure Truth

Remote providers may return typed unsupported/protected/auth statuses. They must
not fake acquisition success, silently fall back to manual import, or enqueue
placeholder files as materialized sources.
