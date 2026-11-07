# RemoteSourceRuntime

`remote_source` owns remote-source provider state, account/session lifecycle,
acquisition jobs, release search/grab (Indexer lane), staging roots, acquired
session files, Supplemental Assets, and staging cleanup.

## Public API Strip

- Provider-neutral command types come from `mod.rs`. `RemoteUiIntent` and
  `RemoteUiSnapshot` (with their connection, Indexer-work, and per-release
  status vocabulary) cross the session boundary. Connection mutation,
  selection, Search/Grab, auth start/completion/disconnect, and library refresh
  cross only `SessionIntent::Remote`. `RemoteLibrarySnapshot` and
  `RemoteAcquisitionSnapshot` are separate revisioned session parts; the
  runtime itself is crate-private.
- Processing, audio, metadata, output artifact, WorkRuntime, and frontend code
  consume provider-neutral facts only (`check-known-mistakes.ts` rejects
  `crate::remote_source` in those engine owners). No provider secrets,
  license blobs, raw provider responses, or protected intermediates cross the
  strip or the generated TypeScript boundary.
- Supplemental Assets cross only as provider-neutral, validated asset facts.
  Processing receives them explicitly by file-list `inputId` and never queries
  `RemoteSourceRuntime`.

## Provider and storage rules

- Credential service names follow the app identifier (`<identifier>.remote-source`)
  so development identities cannot read, replace, or delete production provider
  credentials; isolated profiles need their own sign-in and keys.
- ABB registers its Audible device itself (`providers/audible/registration.rs`)
  as the current Audible for iPhone app: a 40-character alphanumeric serial and
  the app, iOS, and software versions in that file. Why: since October 2026
  Audible denies licenses to the 2021 profile that `audible_api` hard-codes
  (mkb79/Audible#909). A saved sign-in keeps the profile it registered with,
  so a profile change takes effect only after the user signs in again.
- Linux vault choice and its account message: rustdoc in `vault.rs`.
- Staged remote writes (`scoped_output.rs`): `prepare` (stale-partial
  pre-clean) -> partial write -> cancel check -> same-directory
  `rename_and_commit`. Drop removes the partial; the final path appears only
  through that rename, into a fresh item directory. `ProvisionalCommittedFile`
  holds committed audiobook output until validation and supplemental steps
  succeed. Post-download cancel uses `rollback_committed_file`. Cross-device
  rename has no fallback here.
- Indexer grabs create no acquisition jobs and materialize no files into Input.
  Grab logging correlates start, HTTP response, and outcome with a request ID
  and keeps credentials, release GUIDs/URLs, and raw response bodies out of the
  entries. Why: Prowlarr's error text can hold indexer download URLs that carry
  a tracker passkey. A failed Prowlarr reply (grab, search, connection test)
  shows or logs only its status and the body's JSON `message`, cut to 200
  characters with URLs replaced; a non-JSON body gives the status only. A
  failed grab's row shows one line naming the side that is down. Release
  detail URLs are optional source-provided HTTP(S) links without embedded
  credentials, never inferred from a release GUID.
- The Indexer URL and categories are App Settings' `IndexerConnection`, whose
  `validate` rejects embedded credentials on save and draft testing.
  `remote_source` reads them with `SettingsRuntime::indexer_connection` and
  never stores them. API keys live only in the credential store, scoped to the
  normalized server URL. Save reserves its settings turn when accepted and
  changes that URL's key after the change applies; a vault failure says the URL
  and categories changed but the key did not, and a failed save never pairs one
  server with another server's key.
- The private connection owner resolves the saved URL, categories, and the
  host's key for search/grab together; its credential-bearing result never
  crosses IPC.
- A search uses exactly the saved categories. An edit or save that chooses none
  is refused (`empty_categories_refused` on the draft), and a saved unit with
  none resets and is named in recovery (default Audio 3000, Audiobooks 3030).
- Connection Test accepts a draft without persisting it. An omitted draft key
  resolves only from that draft URL's vault slot; a new URL requires its own
  key.
- Audible Supplemental PDFs come from provider-private authenticated
  `GET /companion-file/{title_id}`. Audible API `pdf_url` fields are presence
  hints, not direct-download facts. A requested PDF with no hint imports the
  audio and adds a non-blocking `SupplementalPdfUnavailable` diagnostic. A
  hinted PDF stays required. When the reply is Amazon's sign-in redirect or
  page (`/ap/signin` in the redirect or the first `SIGN_IN_PAGE_SCAN_BYTES`
  of the body), the failure says
  "Sign in to Audible again to get the PDF"; other non-PDF replies keep their
  reason.
- Every Audible API call (library pages, title lookup, license) goes through
  `http.rs` `AudibleApi`: a signed request on the auth's own marketplace
  domain (the client signs the path left after that prefix), a per-request
  timeout, then the status check. On library pages and title lookup 401 or
  403 is `AuthRejected`; on a license request only 401 is, because a 403 can
  refuse one title (entitlement, region) or be a rate limit with a working
  sign-in, so it fails that title only. Any other non-success is a failure,
  never a JSON body read as data. ABB owns the library query
  (`library_request_params`). A failed library page fails the
  whole scan: "No partial library was loaded".
- Audible downloads use clients with a read timeout (`READ_TIMEOUT`). A failed
  reconnect, a broken body, or a short body is retried with doubling waits
  from `RETRY_BASE`, at most `MAX_DOWNLOAD_ATTEMPTS` tries in a row without
  new bytes; bytes arriving reset that count. A failed first connect, an HTTP
  error status, or a resumed reply whose known total size differs from the
  first known total ends the download.

## Handoff

An acquisition's provider work runs behind one `AcquireTitles` function; see
its rustdoc in `session_lifecycle.rs`.

The lifecycle registry (`job_state.rs`) holds each job in one state that only
moves forward: Running, then Validated (files wait for the session) or Settled
(Imported, Removed, Failed, Cancelled). Progress is a display value and never
changes the state; a title that fails is a diagnostic, not a terminal stage.
Snapshot `terminal` is "not Running" and `settled` is "Settled". The provider
returns an `AcquisitionOutcome` that the lifecycle applies to the job it
registered.

A finished job hands its files to the session through the engine-set `Handoff`
and records the outcome on the job. From then on the session decides when the
download goes (`crate::session`, `staged.rs`) and calls `purge_session`, which
drops the lifecycle registry's record of the job. The snapshot's job stays
until a disconnect or the next job replaces it. Cancel does nothing to a job
that already finished, so it cannot remove files the session holds.
Materialized handoff files stay usable after provider logout. A Disconnect
refuses unfinished acquisition/handoff; its scope is on
`RemoteUiIntent::Disconnect`. Job changes update
`SessionUpdate.remote_acquisition` (admission `acquiring` and the current job),
a part with its own revision; download progress is published at most every
100 ms, a stage change at once, and a progress tick carries no other part. The
change that settles the job or rejects the sign-in also carries the remote part.
Why: a tick stays one job's size at any library size and cannot drop a crossing
selection change.

## Working Remote State

- `ui.rs` holds the intents, the wire snapshots, and `UiState`. Its intent match
  is routing; effect execution stays in `RemoteUiRun`. Each lane's private
  facts, predicates (`is_changing`, `is_busy`, `can_disconnect`), and refusal
  wording live in one struct: `AudibleLane` (`audible_lane.rs`: sign-in,
  library, title and PDF choices) and `IndexerLane` (`indexer_lane.rs`:
  normalized connection drafts, search and grab, batch outcomes);
  `indexer_save.rs` owns the Indexer Save and Reset effects on the draft. The
  account read generation is shared: `begin_account_read` and `change_lane`
  are the only places that start a read or switch lanes, so they bump every
  generation they must expire. Accepted runs execute on `EngineTasks`
  through the session, whether or not a host awaits the reply.
- `lane_status.rs` words each lane's status line, called in `UiState::snapshot`
  as `busy` is. `audibleMessage` is a pure ladder over auth, account, library,
  and `settled_job_line`, the one ordering fact: a job's line stays until a
  library load, a sign-in, or a disconnect replaces it, and only the settle
  writes it. A running job's text stays on the acquisition part. On the Indexer
  lane the snapshot's `indexer.message` adds the account failure and connected
  note to the stored last-event line. `auth`, `accountStatus`, and
  `libraryStatus` are engine inputs and never reach the wire.
- SelectLane reads the selected account and, for connected Audible outside an
  acquisition/handoff, its library; repeated entry coalesces pending reads.
  Account and library requests carry generations: replaced lanes and accepted
  disconnects discard both stale success and failure as Superseded. Reattachment
  includes retained library rows. The remote, library, and acquisition parts
  are captured together under the UI guard (`ui_parts.rs`); library revisions
  advance only when rows or diagnostics change. Each part owns its revision
  and its published mark (`Part`); the session holds only the opaque
  `RemoteRevisions` it got from `ui_revisions`.
- `RemoteUiSnapshot.busy` says the shown lane refuses new work now; hosts
  disable that lane's actions from it. An acquisition `busy` change also
  moves the remote part, so the update that settles a job frees the lane.
- Auth start reserves Starting before execution, returns authorization only in
  its initiating reply, and retains AwaitingHandoff without the URL. Completion
  reserves credentials until registration and persistence settle. StartAuth,
  Disconnect, acquisition, and lane replacement are refused during completion.
  `AUTH_REGISTRATION_TIMEOUT` bounds registration, so a stalled Amazon
  endpoint cannot hold the surface until quit. A handoff that cannot be read
  or holds no auth code keeps the pending sign-in: auth returns to AwaitingHandoff with the
  reason in `account_status`, so the user can fix the path without a new
  browser round trip. A completion that took the pending sign-in ends Failed. Shutdown cancels registration before the credential
  commit is admitted and awaits an admitted blocking commit. Keychain reads for Indexer search, grab,
  and Test, the Audible library, and acquisition run on a blocking thread the
  caller awaits (`vault_blocking`, rustdoc). None starts once closing. An Indexer key read for Load is dropped at
  shutdown rather than awaited.
- Accepted disconnect reserves the UI guard's disconnecting fact through vault
  work outside the guard; account_status Running exposes that pending work.
  Credential deletion failure preserves account/library choices and reports
  Failed. Successful deletion updates account truth before staging cleanup, so
  cleanup failure cannot leave a Connected account. Failed unmaterialized jobs
  remain in the lifecycle registry; startup abandoned-session cleanup retries
  their paths.
- Available PDFs start included; refresh preserves explicit exclusion while
  pruning titles that cannot be acquired. A Grab batch captures its releases,
  sends sequentially, and keeps per-release failures for explicit retry. Its
  connection lease spans gaps between requests; Save, Reset, and lane
  replacement are refused until it finishes, and a connection Save and a search
  refuse each other the same way. A refused Reset says to wait; no outcome or
  retry row is lost. An admitted Reset (`reset_lease`) refuses a new Search or
  Grab until it ends, however it ends. A release sent since the last search or
  Save is not sent again (the grab answers "Already sent"): it is claimed in
  `sent_releases` before the send and released again if the send is not
  accepted.
- A release is identified by `RemoteRelease.key`, set once where the Indexer
  adapter maps a hit. Select and Grab intents carry only that key; selection,
  grab outcomes, and `sent_releases` all index by it. `indexer.canGrabSelection`
  (some selected release is not Sent) is computed in `UiState::snapshot`.
- The draft starts from the saved settings; Load reads only whether a key is
  stored for the saved URL. An applied Reset replaces the draft, clears its
  errors, failures, and search results, and rereads the account when the Indexer
  lane is shown (`saved_connection_replaced`). A Test that finishes after it
  changes nothing. A Save refuses Reset (`reset_lease`), so no Save finishes
  after it. Old Test/Search results cannot replace newer requests or changed
  lanes. API keys stay private, and `RemoteUiIntent`'s `Debug` prints no payload;
snapshots report only configured/entered facts.
  The HTTP client initializes on first use, so a session that never uses Indexer
  pays no network setup cost.
- Acquisition admission and disconnect share the remote UI guard. A late library
  reply or terminal publication cannot restore disconnected choices or jobs.
  One unsettled acquisition includes its pending Input handoff, not only
  download.

## Failure Truth

Providers return typed unsupported/protected/auth statuses. An Audible
`AuthRejected` reply (see the scope rule above) on the library or on an
acquisition (`AuthRequired` diagnostic) sets the UI state's sign-in-rejected
fact: the account shows NeedsAuth with the reason, and a later keychain read
cannot turn it back to Connected. Only a completed sign-in or a disconnect
clears it. ABB never deletes the stored sign-in on a refusal. A license reply
with `content_license.status_code` Denied fails that title as `LicenseDenied`
even when it carries a download URL, and the message gives Audible's reason
codes. `license_plan.rs` classifies each reply into one `LicensePlan`
(import-ready, decrypt, or refused with its failure kind and message). A
`CustomerThrottled` reason means a temporary limit or an outdated device
registration, so the message says to sign in again, then retry later. It is never reported as a missing URL.
Acquisition success means real materialized files. Dash/Widevine acquisition
stays unsupported until ABB has its own CDM, MPD/PSSH, and content-key design.
An AAX or AAXC license reply with no usable decryption material fails as
`ProtectedUnsupported` before any audio is downloaded; the key is the only thing
that decides, not the reply's shape. An Audible AAX or AAXC file is materialized
only when its output decodes as audio with FFmpeg's built-in decoder. A wrong
key fails as `MaterializationFailed`, and the message names the decrypt. For
AAXC, helper exit 0 is not that proof.
