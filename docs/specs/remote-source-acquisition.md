# Remote Source Acquisition — Active Spec

## Status

**Local experimental implementation authorized.**

The aligned product target remains ABB-native Audible acquisition. The next
workblock should land a local, owner-tested implementation path and let the
feature prove or disprove itself against the repo owner's Audible account.

This is still not license to be sloppy:

- no GPL source copying or translation
- no Libation/AAXClean sidecar path
- no committed secrets, account material, provider payloads, protected content,
  or live response dumps
- no frontend credential persistence or secret-bearing TS payloads
- no silent fallback or fake acquisition success

If an Audible path fails locally, the implementation should surface a truthful
typed failure and record the exact failure. Do not convert it into a planning
stop condition before attempting the local implementation.

## Mission

Land ABB-native remote source acquisition as a first-class ingress path into the
existing ABB workflow. Audible is the first implemented source, but the
architecture is provider-shaped so future sources do not inherit Audible-specific
assumptions.

Remote acquisition is not a replacement processing path. It materializes
import-compatible local audiobook inputs, imports them through ABB's existing
file-list analysis path, and then the user continues the normal
Import -> Inspect -> Decide -> Preflight -> Process -> Verify workflow.

## Locked Product Target

- Native Audible acquisition is required for the product implementation
  workblock.
- No Libation sidecar, helper app dependency, or "use another tool then import"
  workaround.
- No auto-processing after acquisition. The user still reviews metadata,
  output, and encoder settings before starting ABB processing.
- The user never chooses an acquisition destination. ABB owns acquisition
  staging and session storage.
- The final user output directory remains owned by the existing output and
  processing workflow.
- Multi-title acquisition is first-class.
- Supplemental PDFs are detected when available, default to included, and can be
  opted out per title before acquisition.

## Architecture

Target architecture:

Add an eighth Grey-Box Public API: `RemoteSourceRuntime`.

`RemoteSourceRuntime` owns:

- provider registry and provider capabilities
- account/auth lifecycle
- secret vault access
- library scan and session cache
- acquisition jobs
- source download/materialization
- acquired input session storage
- Supplemental Assets
- cleanup and purge behavior

Audible implementation lives entirely inside a private provider cluster:

- `src-tauri/src/remote_source/providers/audible/`

The public API stays provider-neutral:

- `ProviderId`
- `AccountRef`
- `RemoteTitle`
- `AcquisitionPlan`
- `AcquisitionJob`
- `MaterializedSourceFile`
- `SupplementalAsset`

No Audible/account/provider/protected-content internals leak into processing,
metadata, audio engine, output artifact, file import, logs, events, or generated
TypeScript payloads.

## Local Validation Gate

During implementation, prove whether ABB can materialize an Audible-owned title
into an import-compatible local M4B on the repo owner's machine.

The local validation workblock may:

- inspect public/official docs and provider behavior
- use the repo owner's live Audible account for observation and smoke validation
- use third-party docs as protocol landscape evidence
- use Libation public docs and binary/user-observable behavior as landscape
  evidence
- evaluate license-compatible dependencies and crates
- create isolated throwaway prototypes outside ABB production surfaces when they
  reduce risk or clarify provider behavior

The local validation workblock must not:

- copy GPL source, structure, helper names, comments, or flow
- invoke Libation, AAXClean, or another sidecar as ABB's feature path
- create a frontend credential path
- add fallback/shim behavior
- commit provider secrets, protected content, account material, or live response
  payloads

Local validation result categories:

- **Working locally**: ABB can authenticate, load library data, materialize a
  selected account-owned title, import it into the file list, process it through
  normal ABB flow, and copy selected Supplemental PDFs.
- **Partial local validation**: some provider stage works, but another stage fails
  truthfully with typed status and implementation notes.
- **Not working locally**: no provider stage can be made to work without
  violating the hard hygiene constraints above.

## Current Source Findings

- No public, official Audible acquisition API was found in Amazon/Audible
  developer surfaces. Amazon Music has current public/preview APIs, but those do
  not provide owned-Audible-book acquisition for this feature.
- The maintained third-party `audible` Python documentation identifies Audible's
  API as non-public and documents useful protocol facts: paged library calls,
  `pdf_url` response groups, content license requests, bearer/sign-request
  auth, access-token expiry, refresh tokens, and device registration.
- Libation public docs prove the end-user workflow exists on macOS/Windows/Linux:
  account/library scan, external login path, local audio materialization, and
  attached PDF handling. Libation is GPL-3.0 and is landscape evidence only, not
  an implementation source.
- The implementation must treat provider behavior as unstable/private protocol:
  isolate it inside the Audible provider cluster, avoid leaking assumptions into
  ABB core types, and make unsupported/protected cases typed and observable.

## Provider Capability Shape

The UI asks `RemoteSourceRuntime` what sources are available. Audible is the
only implemented provider initially, but the provider contract should expose
capabilities rather than hardcoded Audible behavior.

Minimum capability fields:

- `authFlow`
- `supportsLibraryScan`
- `supportsPagedScan`
- `supportsTypeaheadFilter`
- `supportsSupplementalPdf`
- `supportsMaterializedAudio`
- `supportsRefresh`
- `requiresLiveSession`
- `knownUnsupportedReasons`

## Auth And Secret Handling

Credentials are toxic state. ABB should touch them only where technically
required.

- Store provider auth/session secrets only through a backend `SecretVault`
  adapter backed by platform credential storage.
- macOS: Keychain.
- Windows: Credential Manager / Credential Locker equivalent.
- Linux: Secret Service/libsecret/KWallet where available.
- Linux fallback must not silently write plaintext credentials. If secure secret
  storage is unavailable, use session-only auth or block with a clear error
  unless a deliberate encrypted local vault design is accepted later.
- External-browser auth is the target shape for credential entry. OAuth-capable
  providers must use Authorization Code + PKCE with desktop loopback redirect
  where accepted.
- Custom URI callbacks are secondary and only acceptable if provider/platform
  constraints make loopback unsuitable.
- Audible may require provider-private device/session auth rather than normal
  public OAuth. That stays inside the Audible private cluster and must still
  avoid privileged webview credential capture.
- No provider password fields in Svelte for this workblock.
- No embedded provider login page in a privileged Tauri webview.
- Frontend sees account state only: connected, needs auth, loading, error.
- No tokens, cookies, license blobs, device material, or secrets in TS payloads,
  logs, events, generated bindings, or UI.
- Credentials persist across app restarts.
- Reauth triggers: no stored creds, refresh failure, revoked/expired refresh
  token, account mismatch, keychain/vault failure, or provider-required login.
- Logout purges credentials, account refs, session cache, staged/acquired files,
  Supplemental Assets, and acquisition jobs.
- Logout never touches final user output files already written to the chosen
  output folder.

## Source Use And Protected-Content Boundary

ABB can use official docs, protocol facts, live account observation, generated
fixtures, compatible-license crates, and black-box behavior comparisons.

ABB must not copy or translate GPL implementation source, distinctive structure,
helper names, comments, or flow from Libation/AAXClean or similar projects.

Specification language should describe the product outcome and provider
boundary, not preserve a provider-specific protected-content recipe. The provider
may only emit a `MaterializedSourceFile` when it can produce an
import-compatible local audio file through implementation work that follows the
source-use rules in this section.
If a required provider step fails locally, the PR should surface a typed failure
and record the exact failure rather than shipping a sidecar, frontend credential
shortcut, unregistered fallback, or fake acquisition path.

Unsupported/protected cases should produce explicit typed outcomes, such as
`ProtectedUnsupported`, not partial files.

## Provider Materialization Boundary

Audible materialization is a private provider concern. The public result is only
`MaterializedSourceFile`.

Locked strategy:

- Implement Audible materialization as a native Rust provider-private cluster.
- Use small, auditable, license-compatible Rust crates for generic building
  blocks such as HTTP, crypto primitives, credential storage, and MP4 parsing or
  editing.
- Do not adopt a whole Audible downloader/decrypter crate as ABB's architecture
  unless implementation research proves it is current, maintained,
  license-compatible, source-use safe, and compatible with ABB's
  provider-neutral Public API.
- Do not invoke external acquisition tools or provider sidecars.
- If provider materialization cannot be completed cleanly, emit a typed
  unsupported/protected outcome and return to design.

Allowed design inputs:

- official public docs where they exist
- third-party docs as protocol landscape evidence
- live account observation owned by the repo owner
- generated/sanitized fixtures
- compatible-license crates or libraries
- black-box comparison with existing tools

Rejected inputs:

- copied GPL source or ported GPL flow structure
- provider secrets in frontend state
- Libation/AAXClean sidecar execution
- raw protected/provider intermediates crossing the public API strip
- fake acquisition paths that only enqueue placeholders

The implementation agent may double-check provider behavior before coding, but
must not redesign the architecture without returning to this alignment layer.

## Library UX

Entry point:

- Button near Add Folder: **Acquire**.
- Panel title: **Acquire Audiobooks**.
- Source selector exists and is provider-shaped, but only Audible is selectable
  until another source is implemented.

First use:

1. User opens Acquire.
2. User connects Audible.
3. User returns to ABB after auth.
4. ABB immediately begins loading the Audible library.
5. Titles stream/populate as provider pages arrive.

Returning use:

1. User opens Acquire.
2. ABB refreshes stored credentials through the vault-backed provider session.
3. ABB immediately begins loading the Audible library.
4. If refresh fails, the panel asks the user to reconnect.

Library behavior:

- Session cache by default.
- Persistent library cache only if implementation discovery proves live scanning
  is materially slow, brittle, or rate-sensitive.
- Manual **Refresh Library** action is available.
- Typeahead/incremental filtering narrows the loaded session library quickly as
  the user types.
- Paged provider loading may continue while typeahead filtering runs over loaded
  results.
- Session state may mark already-acquired items to prevent accidental duplicate
  acquisition, but ABB does not become a library manager.

## Acquisition Lifecycle

State machine:

`Planned -> Acquiring -> Materialized -> Validated -> ImportedToFileList -> FinalizedOutput -> Purged`

Disk areas:

- **Working staging**: protected/source/intermediate provider material. Cleared
  immediately after materialization/import, cancellation, or failure cleanup.
- **Acquired input session store**: provider-materialized/import-compatible M4B
  plus selected Supplemental Assets. Stable only for the current ABB session so
  the existing file list can point at real files.

Lifecycle rules:

- ABB validates the materialized M4B as an audio input before import.
- ABB imports through the existing file import analysis workflow.
- Acquired input files are session workbench artifacts, not durable library
  files.
- Processing failure/cancel keeps acquired files available for retry until app
  close, logout, or manual removal.
- Manual removal from file list purges the acquired input M4B and attached
  Supplemental Assets from session store.
- Successful final processing purges the acquired input M4B and Supplemental
  Assets after final output and supplemental copy succeed.
- App startup/close cleanup purges abandoned acquisition session directories.
- Crash cleanup is best effort; startup cleanup is the durable recovery path.
- Logout purges all account-bound acquisition state and staged/session artifacts,
  but not final output files already written by ABB.

## Supplemental Assets

Repo/code term: **Supplemental Asset**.

UI term: **Supplemental PDF**.

Rules:

- PDF is the first concrete Supplemental Asset kind.
- Source provider detects whether a title has a Supplemental PDF.
- If available, UI clearly signals it and defaults include-on.
- User can opt out per title.
- No PDF means no opt-out control.
- ABB does not mutate, parse, tag, render, or embed PDFs.
- Supplemental Assets attach at file-list item level after acquisition.
- User can remove/detach a Supplemental PDF before processing.
- Batch processing preserves item-to-asset mapping.
- Merge-mode Supplemental Asset copying is intentionally out of scope for this
  workblock.
- Supplemental PDFs copy to the user output directory only after the matching
  final M4B succeeds.
- Preview outputs do not copy Supplemental PDFs.
- Failed/cancelled processing does not copy Supplemental PDFs.
- Validate extension plus PDF magic bytes, cap file size, and keep PDFs out of
  metadata and audio processing.

Handoff shape:

- Use stable generated input IDs for `supplementalAssetsByInputId`.
- Do not key Supplemental Asset ownership by raw filesystem path as a planned
  fallback. Paths are mutable labels, not durable workbench identity.
- No hidden `RemoteSourceRuntime` lookup from processing.

## Workbench Input Identity

Remote acquisition needs stable workbench identity because acquired files and
Supplemental Assets move through file-list, processing, and output surfaces.

Decision:

- Add a backend-generated `inputId` to the analyzed audio-file contract.
- Generate `inputId` during file-list analysis/import.
- Treat `inputId` as session/workbench identity, not a durable library ID.
- Keep path as the filesystem label and processing source path.
- Use `inputId` for Supplemental Asset ownership, UI joins, reorder/remove
  stability, and explicit processing handoff maps.
- Do not expose provider account/title/library IDs as file-list identity.
- Do not use raw path fallback for Supplemental Asset ownership.

## Processing And Output Integration

Processing receives an explicit Supplemental Asset map from the frontend
workflow boundary. It must not discover Supplemental Assets through hidden
remote-source reach-through.

Output artifact ownership should handle final Supplemental PDF naming,
collision review, parent directory creation, and copy/commit behavior.

Default PDF naming candidate:

- If final output is `Book.m4b`, Supplemental PDF candidate is
  `Book - Supplemental PDF.pdf`.

Supplemental PDF collisions follow the same output collision policy selected for
the matching final audio output. PDF naming is derived from the final M4B stem.

## Security Invariants

- Backend commands never return secrets.
- Logs/events/errors redact secret-bearing values by type, not by developer
  memory.
- No remote provider pages/scripts load inside the main app webview.
- Tauri commands validate all provider IDs, account refs, job IDs, paths, and
  asset IDs server-side.
- Filesystem writes stay under ABB-owned staging/session roots or user-selected
  final output roots.
- Cleanup must not follow symlinks out of ABB-owned roots.
- Acquired files and Supplemental Assets carry hash, size, kind, provenance, and
  validation result.
- Provider raw responses and license/session material are not exposed to TS.
- Fallback/shim behavior, if any, must be explicit, observable, registered in
  `docs/fallbacks.md`, and source-marked.

## Tauri Boundary

Remote-source commands are sensitive IPC.

- Add a dedicated remote-source command/permission surface rather than mixing
  auth/acquisition commands into unrelated Tauri command groups.
- Grant sensitive auth/session/acquisition commands only to trusted local ABB
  windows or webviews.
- Remote provider pages must not receive ABB IPC permissions.
- If the Acquire panel becomes its own window later, it still uses local trusted
  app content, not provider-hosted content with ABB IPC.
- Validate provider IDs, account refs, job IDs, asset IDs, and generated
  `inputId` joins server-side.

## External Auth Fallback Policy

ABB should do its level best to make external-browser/native auth work because
that is the expected secure desktop shape and other audiobook tools prove the
user flow is achievable.

If external-browser/loopback auth cannot be made cleanly for Audible, the PR
blocks and returns to design. Do not replace it with embedded webview auth,
Svelte credential fields, cookie-paste flows, plaintext session files, or other
ad hoc shortcuts without explicit re-approval.

## Linux Vault Fallback

If Linux secure credential storage is unavailable, ABB uses session-only auth or
blocks with a clear error. It must not silently fall back to plaintext config.

## Legal And Source-Use Notes

This spec is technical design, not legal advice. The implementation must stay
inside the source-use rules, avoid GPL source copying, and keep the
owner-visible risk explicit
when using private provider behavior or protected-content materialization.

Libation and similar tools are valid evidence that the user workflow is
achievable. They are not implementation source.

## Verification Strategy

Keep implementation notes while working. Record sources consulted, dependency
and license findings, live-account observations, and exact provider-stage
failures. Prototypes must stay throwaway or clearly isolated from ABB production
code, with no committed secrets, protected-content artifacts, or live provider
payloads.

Required verification categories:

- Rust contract tests for `RemoteSourceRuntime` Public API behavior.
- Dependency/license note for the provider-private materialization dependencies
  chosen during implementation.
- Provider fake tests using sanitized fixtures, not live network.
- Vault adapter mock tests and platform adapter tests where feasible.
- IPC/generated-binding checks proving no secret-bearing types cross to TS.
- Log canary tests proving fake token/password/license strings do not appear in
  logs/errors/events.
- Path traversal and symlink cleanup tests for staging/session roots.
- Crash/startup cleanup tests for abandoned acquisition session directories.
- Protected/unsupported asset tests proving no `MaterializedSourceFile` is
  emitted.
- Frontend workflow tests for auth states, library loading, typeahead filtering,
  multi-select acquisition, Supplemental PDF default/opt-out, and imported-file
  handoff.
- UI tests proving Supplemental PDFs never enter audio processing.
- Manual live-account smoke test against the owner's Audible account.

Required local commands before PR:

- `cargo fmt --all -- --check`
- `bun run fmt:check`
- `bun run lint:check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `bash scripts/check-generated-bindings.sh --mode local`
- `bash scripts/check-public-api-strips.sh`
- `bash scripts/check-no-bridge-imports.sh`
- `bash scripts/check-fallback-policy.sh`
- `cargo nextest run -p abb-remote-source-core`
- `cargo nextest run -p audiobook-boss --lib`
- `bun run test -- src/lib/tauri-client.test.ts src/lib/tauri-public-api.contract.test.ts src/lib/behavior-contract.test.ts src/ui/remoteSource/sessionAssets.test.ts`
- `bun run build`

Required live smoke evidence before done:

- Authenticate to Audible through ABB.
- Load library and select at least one title.
- Acquire one Audible title through ABB.
- Include a Supplemental PDF if an available title has one.
- Import the acquired M4B into the file list.
- Process to final output through the normal ABB flow.
- Verify final M4B and Supplemental PDF in the selected output directory.
- Logout/purge and verify acquired session artifacts are removed.
- Confirm no secrets appear in relevant logs/events.

## Implementation Handoff Checks

Before product coding, the implementation agent should turn this spec into a
concrete plan that includes:

- generated `inputId` changes across file list, processing request composition,
  output commit, and verification coverage
- provider-private dependency/license note for Audible materialization
- Tauri permission/capability files or config touched by remote-source commands
- implementation scratchpad entries for any provider behavior discovered during
  live-account validation

This product handoff is authorized for local implementation and owner testing.

## Research References

- Audible third-party API docs:
  <https://audible.readthedocs.io/en/latest/>
- Audible external API notes:
  <https://audible.readthedocs.io/en/master/misc/external_api.html>
- Audible auth notes:
  <https://audible.readthedocs.io/en/latest/auth/authentication.html>
- Libation public docs:
  <https://github.com/rmcrackan/Libation/tree/master/docs>
- Libation license:
  <https://github.com/rmcrackan/Libation/blob/master/LICENSE>
- RFC 8252 native app OAuth guidance:
  <https://www.rfc-editor.org/rfc/rfc8252>
- RFC 9700 OAuth security best current practice:
  <https://www.ietf.org/rfc/rfc9700.html>
- Tauri v2 capability/permission model:
  <https://v2.tauri.app/reference/acl/capability/>

## Cleanup Trigger

When the work is implemented, reviewed, validated, documented, and synced,
delete this active spec or distill only enduring ownership rules into
`docs/system-map.md`, `docs/api-map.md`, `docs/ubiquitous-language.md`, nearest
`AGENTS.md`, and `docs/fallbacks.md` if a registered fallback exists.
