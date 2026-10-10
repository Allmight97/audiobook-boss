# RemoteSourceRuntime

`remote_source` owns remote-source provider state, account/session lifecycle,
acquisition jobs, release search/grab (Indexer lane), staging roots, acquired
session files, Supplemental Assets, and staging cleanup (the session decides
when an imported download goes).

## Public API Strip

- Provider-neutral command types come from `mod.rs`. `RemoteUiIntent` and
  `RemoteUiSnapshot` (with their connection, Indexer-work, and per-release
  status vocabulary) cross the session boundary. Connection mutation,
  selection, Search/Grab, auth start/completion/disconnect, and library refresh
  cross only `SessionIntent::Remote`. `RemoteLibrarySnapshot` is a separate
  revisioned session part; the runtime itself is crate-private.
- Processing, audio, metadata, output artifact, and frontend code consume
  provider-neutral facts only. No provider secrets, license blobs, raw provider
  responses, or protected intermediates cross the strip or the generated
  TypeScript boundary.
- Supplemental Assets cross only as provider-neutral, validated asset facts.
  Processing receives them explicitly by file-list `inputId` and never queries
  `RemoteSourceRuntime`.

## Provider and storage rules

- Credential service names follow the app identifier so development identities
  cannot read, replace, or delete production provider credentials. The
  production identifier keeps its shipped service name; isolated profiles need
  their own sign-in and keys.
- On Linux the vault uses the Secret Service when one answers on the session
  bus, and kernel keyutils otherwise. Keyutils lasts until restart; the account
  message says sign-in will not be remembered. The warning follows the store's
  persistence (`UntilDelete` survives). No second copy of a secret is written.
- Staged remote writes (`scoped_output.rs`): `prepare` (stale-partial
  pre-clean) -> partial write -> cancel check -> same-directory
  `rename_and_commit`. Drop cleans uncommitted paths. `ProvisionalCommittedFile`
  holds committed audiobook output until validation and supplemental steps
  succeed. Post-download cancel uses `rollback_committed_file`. Cross-device
  rename has no fallback here.
- Indexer grabs create no acquisition jobs and materialize no files into Input.
  Grab logging correlates start, HTTP response, and outcome with a request ID
  and keeps credentials, release GUIDs/URLs, and raw response bodies out of the
  entries. Release detail URLs are optional source-provided HTTP(S) links
  without embedded credentials, never inferred from a release GUID.
- Indexer connection URLs reject embedded credentials on save and draft
  testing. The URL and categories live in `indexer.toml` under App Settings'
  storage rules: written crash-safe, a failed write logged and reported; a file
  damaged outside ABB (including a URL with credentials) loads as not
  configured and never reaches IPC or a provider. API keys stay in the
  credential store.
- The private connection owner resolves URL, categories, and the host's key for
  search/grab together; its credential-bearing result never crosses IPC.
- A search uses exactly the saved categories. An edit or save that chooses none
  is refused (`empty_categories_refused` on the draft), and a file with none
  loads as the default (Audio 3000, Audiobooks 3030).
- Indexer credentials are scoped to the normalized server URL in the vault;
  connection JSON never holds a key. Save persists changed JSON before changing
  that URL's key, and reports partial persistence if the vault fails. A failed
  save never pairs one server with another server's key.
- Connection Test accepts a draft without persisting it. An omitted draft key
  resolves only from that draft URL's vault slot; a new URL requires its own
  key.
- Audible Supplemental PDFs come from provider-private authenticated
  `GET /companion-file/{title_id}`. Audible API `pdf_url` fields are presence
  hints, not direct-download facts. A requested PDF with no hint imports the
  audio and adds a non-blocking `SupplementalPdfUnavailable` diagnostic.

## Handoff

A finished job hands its files to the session through the engine-set `Handoff`
and records the outcome on the job. From then on the session decides when the
download goes (`crate::session`, `staged.rs`) and calls `purge_session`, which
also drops the job's record. Cancel does nothing to a job that already
finished, so it cannot remove files the session holds. Materialized handoff
files stay usable after provider logout. An Audible disconnect refuses
unfinished acquisition/handoff; an Indexer disconnect neither waits for nor
cleans up Audible jobs. Job changes update `SessionUpdate.remote`; download
progress is published at most every 100 ms, a stage change at once. Snapshot
`terminal` and `settled` facts belong here.

## Working Remote State

- `ui.rs` owns accepted title/PDF/release choices, normalized connection drafts,
  readback generations, and batch outcomes. Its intent match is routing; effect
  execution stays in `RemoteUiRun`. Accepted runs execute on `EngineTasks`
  through the session, whether or not a host awaits the reply.
- SelectLane reads the selected account and, for connected Audible outside an
  acquisition/handoff, its library; repeated entry coalesces pending reads.
  Account and library requests carry generations: replaced lanes and accepted
  disconnects discard both stale success and failure as Superseded. Reattachment
  includes retained library rows; progress events carry only the remote part.
  Both parts are captured together under the UI guard; library revisions advance
  only when rows or diagnostics change.
- Auth start reserves Starting before execution, returns authorization only in
  its initiating reply, and retains AwaitingHandoff without the URL. Completion
  reserves credentials until registration and persistence settle. StartAuth,
  Disconnect, acquisition, and lane replacement are refused during completion.
  Shutdown cancels registration before the credential commit is admitted and
  awaits an admitted blocking commit. Started keychain reads are awaited too;
  none starts once closing, so a system prompt cannot hold quit.
- Accepted disconnect reserves the UI guard's disconnecting fact through vault
  work outside the guard; account_status Running exposes that pending work.
  Credential deletion failure preserves account/library choices and reports
  Failed. Successful deletion updates account truth before staging cleanup, so
  cleanup failure cannot leave a Connected account. Cleanup covers only the
  disconnected provider's jobs. Failed unmaterialized jobs remain in the
  lifecycle registry; startup abandoned-session cleanup retries their paths.
  Materialized handoff files remain session-owned.
- Available PDFs start included; refresh preserves explicit exclusion while
  pruning titles that cannot be acquired. A Grab batch captures its releases,
  sends sequentially, and keeps per-release failures for explicit retry. Its
  connection lease spans gaps between requests; Save and lane replacement are
  refused until it finishes, and a connection Save and a search refuse each
  other the same way. A release sent since the last search or Save is not sent
  again (the grab answers "Already sent").
- Connection readback cannot erase newer typing or a newer Save. Old Test/Search
  results cannot replace newer requests or changed lanes. API keys stay private;
  snapshots report only configured/entered facts. The HTTP client initializes on
  first use, so a session that never uses Indexer pays no network setup cost.
- Acquisition admission and disconnect share the remote UI guard. A late library
  reply or terminal publication cannot restore disconnected choices or jobs.
  One unsettled acquisition includes its pending Input handoff, not only
  download.

## Failure Truth

Providers return typed unsupported/protected/auth statuses. Acquisition success
means real materialized files: manual-import fallback and placeholder sources
do not exist. Dash/Widevine acquisition stays unsupported until ABB has its own
CDM, MPD/PSSH, and content-key design. An Audible AAX or AAXC file is
materialized only when its output decodes as audio with FFmpeg's built-in
decoder. A wrong key fails as `MaterializationFailed`, and the message names
the decrypt. For AAXC, helper exit 0 is not that proof.
