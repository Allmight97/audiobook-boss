# Remote Source

- No provider secret, license blob, raw provider response, or protected
  intermediate crosses into the session snapshot or the generated TypeScript.
  The connection owner's credential-bearing result stays out of IPC types.
- Credential service names follow the app identifier
  (`<identifier>.remote-source`), so a development identity cannot read,
  replace, or delete production credentials; an isolated profile needs its
  own sign-in and keys.
- ABB registers its Audible device as the current Audible for iPhone app
  (`providers/audible/registration.rs`), because since October 2026 Audible
  denies licenses to the 2021 profile (mkb79/Audible#909). A saved sign-in
  keeps the profile it registered with until the user signs in again.
- A saved Audible sign-in is keychain JSON in the format earlier releases
  wrote; changing `Auth`'s serialized shape signs users out.
- Grab and Indexer logs and failure text keep credentials, release GUIDs and
  URLs, and raw response bodies out, because Prowlarr's error text can hold
  download URLs that carry a tracker passkey. A failed reply shows only its
  status and the body's JSON `message`, cut to 200 characters with URLs
  replaced.
- Release detail URLs are optional source-provided HTTP(S) links without
  credentials, never inferred from a release GUID.
- Indexer API keys live only in the credential store, scoped to the
  normalized server URL; a failed Save never pairs one server with another
  server's key.
- Audible companion PDFs come from the authenticated
  `GET /companion-file/{title_id}`; the API's `pdf_url` fields are presence
  hints, not download facts. A requested PDF with no hint imports the audio
  with a `SupplementalPdfUnavailable` diagnostic; a hinted PDF stays required.
  An Amazon sign-in redirect or page in the reply means "Sign in to Audible
  again to get the PDF".
- On a license request only 401 is `AuthRejected`, because a 403 there can
  refuse one title (entitlement, region) or be a rate limit with a working
  sign-in; it fails that title only. On library pages and title lookup, 401 or
  403 is `AuthRejected`. A failed library page fails the whole scan, so no
  partial library loads.
- Cancel or failure of a running acquisition purges its staging folder with
  `purge_with_retries`, because an aborted decrypt only signals its blocking
  worker to stop and the worker may still touch the folder.
- A download progress tick carries only the acquisition part, so it stays one
  job's size at any library size and cannot drop a crossing selection change.
- `begin_account_read` and `change_lane` are the only places that start an
  account read or switch lanes, so they bump every generation they must
  expire.
- Hosts disable a lane's actions from `RemoteUiSnapshot.busy`; an acquisition
  `busy` change also moves the remote part, so the update that settles a job
  frees the lane.
- `AUTH_REGISTRATION_TIMEOUT` bounds device registration, so a stalled Amazon
  endpoint cannot hold the surface until quit.
- A disconnect updates account truth before staging cleanup, so a cleanup
  failure cannot leave a Connected account.
- The Indexer URL draft keeps the text as typed and only Test and Save
  normalize it, because rewriting the field under the user's cursor loses
  keystrokes.
- The draft snapshot supplies `category_options` and `uses_http`; the
  Settings form holds no category list and no URL rule.
- Acquisition admission and disconnect share the remote UI guard, so a late
  library reply or terminal publication cannot restore disconnected choices or
  jobs.
- An Audible `AuthRejected` reply marks the sign-in rejected until a completed
  sign-in or a disconnect; a later keychain read cannot show Connected, and ABB
  never deletes the stored sign-in on a refusal.
- A license reply whose `content_license.status_code` is Denied fails that
  title as `LicenseDenied` even with a download URL. A `CustomerThrottled`
  reason means a temporary limit or an outdated device registration, so the
  message says to sign in again, then retry later.
- Dash and Widevine acquisition stays unsupported until ABB has its own CDM,
  MPD/PSSH, and content-key design.
- The decryption key alone decides an AAX or AAXC lane, not the reply's shape.
  For AAXC, demuxer success does not prove the key: the decrypt decodes the
  partial with FFmpeg's built-in decoder and reads the chapters back before
  the rename.
- Accepted risk: FFmpeg keeps plain copies of key material (option strings and
  the `mov.c` context) that ABB cannot wipe; an AAXC key opens one title, and
  AAX activation bytes stay in process memory only.
- Known gap, accepted: the AAX lane is proven only on test books built with
  FFmpeg's algorithm. No known title gets AAX and every real download so far
  was AAXC; a review does not re-flag it.
- Decrypt honors the download's edit list, because it is Audible's statement
  of the book's length and its chapters end there. A Keep-original-audio merge
  of several decrypted titles is therefore refused by the preserve join's
  trim rule.
