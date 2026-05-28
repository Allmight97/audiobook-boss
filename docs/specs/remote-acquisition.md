# Remote Acquisition — Active Spec

## Purpose / Big Picture

ABB becomes an all-in-one acquire-decrypt-process tool for audiobook content. Users authenticate to a remote source (Audible first, more later) inside ABB, pick titles from their library, and ABB acquires + decrypts to local M4Bs which enter the existing manual Import → Inspect → Decide → Preflight → Process → Verify flow.

This borrows Libation's outcome (account-owned remote entitlement → verified local M4B) without becoming a library manager. Existing metadata handling (embedded tags + `search_online_metadata` lookup + draft UI) is sufficient; acquisition produces files, not metadata authoring.

What good looks like: a user logs into Audible inside ABB, selects three titles, watches phase-truthful progress, and lands those titles in the normal ABB file list ready for the usual workflow — with no second tool, no manual file moves, no token leakage to the UI, and no acquisition state leaking into processing.

## Scope And Constraints

### In Scope

- Authenticate to Audible (Amazon Login With Amazon flow; capture `/ap/maplanding` redirect; persist tokens to macOS Keychain).
- Audible device registration (Android-shaped) for Widevine eligibility.
- Widevine device identity resolution: ABB maintains a private `WidevineDeviceProvider` that loads a cached validated device identity, refreshes it from a configured resolver/index when missing or invalid, and supports user-supplied override/import. The device identity is provider-private acquisition infrastructure, not user-facing account state.
- Library discovery with pagination + retry, scoped to a single account.
- Acquisition for **AAXC** (AES-128-CBC with per-title voucher key+IV) and **Widevine** (AES-128-CTR with content key extracted from a Widevine license response). These two cover Audible's current active distribution.
- Atomic commit of decrypted M4B into ABB-owned staging, then handoff via a shared `LocalImportBridge` into the existing file list.
- "Source acquisition format" field surfaced on the file inspector for acquired (or Libation-imported) files: `Audible Widevine (DASH)`, `Audible AAXC`, `Audible AAX (legacy)`, or `unknown`. Read from the `AUDIBLE_DRM_TYPE` freeform tag when present (Libation writes it) or from provenance for native-acquired files.
- Provider trait extensible to non-Audible sources later, with Audible as the first concrete provider.
- Logout that atomically purges keychain, library cache, staging, job ledger, and in-flight acquisition state for that account; revoke server-side device registration when the protocol supports it.

### Non-Goals

- Library management features (shelves, ratings, listening history, "read/unread," watchlists, backup policy). ABB is not Libation.
- Metadata enrichment from Audible API written into the M4B at acquisition time. Existing embedded tags (`Title`, `Album`, `Performer`, `Composer`, `Publisher`, `AUDIBLE_ASIN`, embedded high-res cover, named chapters) are already sufficient input for ABB's existing metadata pipeline. Evidence: the three Libation-decrypted test files in `~/Music/Libation/Books/` all carry rich embedded metadata + 2400×2400 cover + named chapters.
- AAX legacy (activation-bytes AES-CBC). Three-for-three evidence that current Audible distribution serves Widevine even for legacy content (1989 source date re-encoded to USAC/Widevine 2026). Can be added later if a real user hits a title Audible still serves as AAX.
- Auto-processing after acquisition. User always retains manual control of inspect/decide/process. A separate "auto-process after import" setting can live in processing settings later — acquisition is causally ignorant of it.
- iOS/web/non-macOS targets.

### Constraints (Forbidden Paths)

1. No tokens, license blobs, decrypt keys, device private keys, Widevine device identity blobs, or callback secrets reach the frontend. UI sees opaque account refs, title IDs, and phase status only.
2. No DRM internals, account refs, remote URLs, or provider-specific data enter `ProcessPayload` or any processing-pipeline contract. Processing remains local-file + user-intent.
3. No provider-supplied output paths cross the materialized-asset gate. Staging root, filename, and final commit path are ABB-owned. Provider manifests are advisory.
4. Acquisition never causes processing. The user must inspect/decide/preflight/process manually.
5. Provider metadata does not enter the Metadata Outcome Plan directly. Embedded tags surface through normal file analysis; the existing online lookup pipeline remains the metadata authority.
6. Logout is atomic: keychain entry, library cache, staging artifacts, job-ledger entries, and in-flight acquisitions for that account are purged together; partial purge is a recoverable error state.
7. **GPL contamination rule for agents**: code in `src-tauri/src/remote_source/providers/audible/` is implemented from protocol observation, public docs, behavior tests, and the behavioral map captured in this spec. Reading GPL-licensed Libation/AAXClean source files (`/Users/jstar/Projects/tmp_repos/Libation`, `/Users/jstar/Projects/tmp_repos/AAXClean`) while authoring ABB Audible-provider code is prohibited. Studying *protocol* (URLs, HTTP shapes, key derivations described in comments/specs) is allowed; copying *implementation* (file structure, distinctive function shapes, comment phrasing) is not.

## Solution Posture

**Subsystem addition.** Two new grey-box modules, one new provider concrete, no changes to processing/metadata/output-artifact internals.

| Module | Type | Public API Strip | Notes |
| --- | --- | --- | --- |
| `RemoteSourceRuntime` | Rust grey-box (new Public API after ABB's seven current Public APIs) | `providers()`, `accountStatus(providerId)`, `beginAuth/completeAuth/logout(providerId)`, `scanLibrary(accountRef, cursor)`, `preflightAcquisition(plan)`, `startAcquisition(plan)`, `cancelAcquisition(jobId)`, `acquisitionStatus(jobId)`, `commitMaterializedAssets(jobId)` | Provider registry, vault adapter, library cache, acquisition job ledger, staging root, provenance writer, capability matrix. Audible provider lives inside as a private cluster. |
| `LocalImportBridge` | TS grey-box | `importLocalAudioFiles(paths)` returning `FileListInfo` | Extracted from `processFilePaths` in `src/ui/fileImport/handlers.ts:153-171`. Used by picker, drop, AND remote-source UI. Enforces "remote import is not a privileged bypass." |

**Why this posture**: Native acquisition with the trait shape ABB owns end-to-end. No sidecar invocation, no bundled .NET worker, no embedded-binary GPL dependency. The boundary that protects processing's purity is the `MaterializedAsset` handoff — a verified local M4B path + provenance manifest, nothing more.

**Rejected postures**:

- *User-installed sidecar invocation (Libation as CLI)*: rejected. Adds install friction and contradicts the all-in-one product premise. Audible-API drift is absorbed by ABB either way; routing it through Libation's release cadence buys time, not durability.
- *Bundled .NET worker (AAXClean direct)*: rejected. AAXClean is GPL-3 (not MIT as initially assumed), so no clean license escape. Distribution complexity (signed .NET runtime, notarization, updater, IPC contract surface) ABB doesn't otherwise need.
- *Auto-process after acquisition*: rejected per constraint #4.
- *Metadata enrichment seam (`RemoteMetadataObservation`)*: rejected by YAGNI. Embedded tags + existing online lookup cover the metadata need without a new ownership domain.

**What would justify broadening scope**: a real user title that Audible serves as classic AAX (not Widevine). Triggers an `audible.aax_legacy` decrypt path addition. Until evidence appears, AAX support is out.

## Context And Orientation

### New Owned Paths

- Rust: `src-tauri/src/remote_source/` (runtime), `src-tauri/src/remote_source/providers/audible/` (Audible-private cluster)
- Rust commands: `src-tauri/src/commands/remote_source.rs`, registered in `src-tauri/src/ipc_contract.rs`
- Rust AGENTS.md: `src-tauri/src/remote_source/AGENTS.md` (Public API Strip, allowed edits, breaking-change triggers, GPL contamination rule, Vault reach-through prohibition)
- Frontend: `src/ui/remoteSource/` (account state, library picker, acquisition job tray)
- Shared local import: `src/lib/import/localImport.ts` (extracted from `src/ui/fileImport/handlers.ts:153-171`)
- Boundary assertion: `scripts/check-no-remote-source-reach-through.sh` (CI gate)

### Touched Existing Surfaces

- `src/ui/fileImport/handlers.ts:153-171` — `processFilePaths` extraction. Callers updated to import from `src/lib/import/localImport.ts`. Behavior identical.
- `src-tauri/src/ipc_contract.rs` — register new command/event families.
- `src/lib/tauri/client.ts` — adapter additions for new commands/events.
- `docs/api-map.md` — add Remote Source command/event family.
- `docs/system-map.md` — add Remote Source row to the current seven Public APIs if this spec is implemented.
- `docs/ubiquitous-language.md` — add: Remote Acquisition Plane, Materialized Asset, Acquisition Job Ledger, Vault Adapter, Provider Driver, Source Acquisition Format.
- `docs/fallbacks.md` — register Widevine/AAXC-related compat paths if any emerge during D5/D6 with explicit triggers, signals, and sunsets.
- File inspector display (existing UI component currently showing Codec/Decoder/Bitrate/etc.) — add "Source" row.

### Invariants Protected

- `ProcessPayload.input_files` stays `Vec<String>` of local paths; no remote shape.
- Metadata Outcome Plan stays the single metadata boundary for source hydration, `set/clear/noop` intent projection, naming-safe metadata, write instructions, and cover-art passthrough policy.
- `validate_input_audio_path` runs on every committed materialized asset before it enters `FileListInfo`.
- `Specta` remains source of TS/Rust contract truth; no hand-edited bindings.
- Terminal-outcome shape extends without breakage: `success`, `cancelled`, `failed` apply to acquisition jobs (no new categories).
- Existing `JobRegistry` is NOT shared with acquisition. A peer `AcquisitionJobRegistry` owns acquisition lifecycle. Reuse would create an Ownership Smear (two job-lifecycle definitions in one registry).

## Plan Of Work

### Phase A — Foundations (no Audible code yet)

- **A1**. Extract `LocalImportBridge` from `handlers.ts:processFilePaths` into `src/lib/import/localImport.ts`. Update picker + drop callers. Add behavior test that proves identical output for the same inputs.
- **A2**. Scaffold `src-tauri/src/remote_source/`: provider trait, `RemoteSourceRuntime` skeleton, `Vault` trait, `AcquisitionJobRegistry` skeleton, `MaterializedAsset` + `ProvenanceManifest` types, phase event enum. Wire empty command stubs through `ipc_contract.rs`.
- **A3**. Write `src-tauri/src/remote_source/AGENTS.md`: Public API Strip enumeration, Private Cluster enumeration, allowed agent edits, breaking-change triggers, GPL contamination rule, Vault reach-through prohibition with target script.
- **A4**. Add boundary-assertion script `scripts/check-no-remote-source-reach-through.sh` modeled on `scripts/check-no-bridge-imports.sh` (existing pattern). Blocks: imports of `remote_source::providers::*::*` from outside the provider module; imports of `remote_source::vault::*` from anywhere outside `RemoteSourceRuntime`'s private cluster. Wire into `bun scripts/proof/runner.ts review`.
- **A5**. Update `docs/system-map.md` (eighth Public API row + Boundary section), `docs/api-map.md` (new command family), `docs/ubiquitous-language.md` (new terms).

### Phase B — Audible auth

- **B1**. Vault implementation via `keyring` crate against macOS Keychain. One entry per account, opaque to callers.
- **B2**. External-browser-first login: launch Amazon LWA URL via `tauri-plugin-opener`; loopback HTTP listener on `127.0.0.1` catches the `/ap/maplanding` redirect with auth code. Fallback to in-app webview only if loopback proves unreliable.
- **B3**. Authorization-code exchange against Amazon LWA token endpoint. Persist response payload (`adp_token`, `access_token`, `refresh_token`, `device_private_key`, `store_authentication_cookie`, `device_info`, `customer_info`, `expires`, `locale_code`) to Vault under opaque AccountRef.
- **B4**. Device registration: Android-shaped device claim against Audible registration endpoint. Required for Widevine eligibility.
- **B5**. Audible API request signing using `device_private_key` (RSA-SHA256 of canonical request headers; observed shape — agent must derive specifics from live traffic, not from Libation source).
- **B6**. Token refresh on 401/expiry; updates Vault atomically.
- **B7**. Logout: server-side device deregistration (when supported), then atomic local purge (Vault + cache + staging + ledger).
- **B8**. Multi-marketplace: `Locale` per account; per-marketplace endpoint resolution (`api.audible.com`, `.co.uk`, `.de`, etc.).

### Phase C — Library discovery

- **C1**. `GET` paged library endpoint with concurrency cap (8) + 2 retries on transient failure. Returns provider-neutral `RemoteTitle` objects (ASIN, title, authors, narrators, runtime, cover URL, locale, capability hint).
- **C2**. Library cache (per account) with explicit invalidation and TTL. Survives crashes; respects logout purge.
- **C3**. `RemoteSourceUI` library picker: account header, marketplace selector, title list with filtering, multi-select, "acquire selected" action.

### Phase D — Acquisition (AAXC + Widevine)

- **D1**. License request: `GET /content/{asin}/licenserequest`. Classify response into AAXC vs Widevine via key shape (mirrors Libation's `DownloadOptions.cs:62-74` decision tree, derived independently from license-response field shapes — NOT copied):
  - `DrmType=Adrm`, key length 16, iv length 16 → AAXC
  - `DrmType=Widevine` → Widevine route (extract MPD URL + Widevine license-response payload)
  - Anything else (including `DrmType=Adrm` with 4-byte key, i.e. AAX legacy) → unsupported in v1, fail with explicit user-visible error
- **D2**. **AAXC decrypt**: streaming HTTPS GET with Range-resume into staging temp file; open as MP4 reader; AES-128-CBC frame decrypt with voucher key+IV; remove `adrm` + DRM-related boxes from moov; verify resulting MP4 structure parses as valid M4B (one audio track, AAC/USAC, expected sample count vs declared duration).
- **D3**. **Widevine decrypt**: HTTPS GET on resolved MPD `BaseURL` (single concatenated URL pattern per Audible; segment splicing not needed); open MP4 with CENC `sinf`/`pssh` boxes; parse Widevine LicenseResponse protobuf to extract AES content key; AES-128-CTR streaming decrypt of CENC samples; strip CENC protection atoms; verify resulting M4B.
- **D4**. **Widevine device identity**: implement a private `WidevineDeviceProvider` inside the Audible provider cluster.
  - First load a cached validated device identity from the provider-private store.
  - If cache is absent or invalid, fetch a configured resolver/index, try resolver endpoints using an Android-registered Audible account proof, validate the returned WVD/device bytes by parsing the header/client ID/private key and constructing a license challenge, then cache the bytes with source URL, retrieval timestamp, and SHA-256.
  - Support a user-supplied WVD/device identity import as an override/fallback.
  - Do not vendor a device blob into ABB release artifacts. Do not depend on Libation's `.cdmurls.json` as an unowned canonical service; Libation's shape is evidence for the mechanism, not ABB's infrastructure contract.
  - If no valid device identity is available, Widevine acquisition fails explicitly as `widevineDeviceUnavailable`; do not silently downgrade the title into another acquisition route unless the license response itself supports AAXC.
- **D5**. Acquisition job lifecycle: phase enum `{ libraryScan, licenseRequest, download, decrypt, verify, stageCommit }`; per-phase progress where measurable (download bytes, decrypt sample count); cancellation at any phase with cleanup; one terminal outcome per job (`success`/`cancelled`/`failed`).
- **D6**. Staging: ABB-owned root under `$DATA_DIR/abb/remote-source-staging/<accountRef>/<jobId>/`; generated filenames (UUID-based, no provider data in path); `.partial` suffix during writes; atomic rename on completion; rejected if any path escapes staging root.
- **D7**. Provenance manifest: ASIN, ACR, source DRM type, locale, acquisition timestamp, integrity hash (SHA-256 of decrypted M4B), provider version. Persisted as sidecar JSON next to staged file; copied into a side-table indexed by canonical path at commit.
- **D8**. Commit revalidation: run `validate_input_audio_path` on the final canonical path before calling `LocalImportBridge`; reject + clean up on validation failure.

### Phase E — Import handoff + UI

- **E1**. After successful commit, `RemoteSourceUI` calls `LocalImportBridge.importLocalAudioFiles([committedPath])`. Provenance reference attaches via the side-table indexed by canonical path; no new IPC needed for analysis. User lands in the standard file list with the file already added.
- **E2**. Acquisition Job UI: real-time phase rendering, cancel button, per-job error display, "import-ready" affordance (auto-handoff on default; toggle for "stage only, don't import").
- **E3**. File inspector "Source" row: reads from provenance side-table first; falls back to `AUDIBLE_DRM_TYPE` embedded tag; falls back to codec heuristic (USAC ⇒ likely Widevine; AAC-LC 22 kHz no `AUDIBLE_DRM_TYPE` ⇒ likely classic AAX); displays `Audible Widevine (DASH)` / `Audible AAXC` / `Audible AAX (legacy)` / `unknown` accordingly. Wording is acquisition provenance, not current DRM state (the file is unencrypted).
- **E4**. Logout UI; confirmation dialog enumerating purge scope.

### Phase F — Validation and docs

- **F1**. Contract tests for `RemoteSourceRuntime` Public API Strip behavior. Lock the strip; internal cluster changes must keep them green.
- **F2**. Boundary-assertion CI: `scripts/check-no-remote-source-reach-through.sh` must be green in `bun scripts/proof/runner.ts review`.
- **F3**. Drift fixture corpus committed under `src-tauri/tests/fixtures/audible/`: sanitized library response, AAXC license response, Widevine license response, MPD manifest, ADRM activation response (for future AAX), at least one failure-class fixture per phase. Provider tests run from fixtures, not live network.
- **F4**. Real-account smoke test runbook: documented manual procedure to acquire one AAXC title (if available) and one Widevine title against jstar's account; recorded in the spec progress section.
- **F5**. `docs/system-map.md`, `docs/api-map.md`, `docs/ubiquitous-language.md` reflect committed state.
- **F6**. `bun scripts/proof/runner.ts review` green.
- **F7**. Delete this active spec or distill enduring truths into canon once
  implementation, review, validation, docs alignment, and sync are complete.

## Interfaces And Dependencies

### New IPC Commands (registered in `ipc_contract.rs`)

```text
remote_source::list_providers() -> Vec<ProviderDescriptor>
remote_source::account_status(provider_id) -> AccountStatus
remote_source::begin_auth(provider_id, marketplace) -> AuthSession
remote_source::complete_auth(auth_session_id, callback_data) -> AccountRef
remote_source::logout(account_ref) -> ()
remote_source::scan_library(account_ref, cursor?, filters?) -> LibraryPage
remote_source::preflight_acquisition(plan) -> AcquisitionPreflight
remote_source::start_acquisition(plan) -> JobId
remote_source::cancel_acquisition(job_id) -> ()
remote_source::acquisition_status(job_id) -> AcquisitionStatus
remote_source::commit_materialized_assets(job_id) -> Vec<MaterializedAssetRef>
```

### New Events

- `acquisition-progress` — phase + percent-or-counter per job
- `acquisition-terminal` — final outcome per job
- `auth-state-changed` — account session state transitions
- `library-scan-progress` — pagination/scan progress

### Public Result Types

```text
MaterializedAssetRef:
  - canonical_path: String      (validated absolute path)
  - asin: Option<String>
  - source_drm_type: Option<SourceDrmType>   (AaxLegacy | Aaxc | Widevine)
  - acquisition_timestamp: ISO8601
  - integrity_sha256: String
  - provider_id: String

ProvenanceManifest:
  - All fields above PLUS:
  - locale: String
  - acr: Option<String>
  - provider_version: String
  - acquisition_diagnostics: Vec<AcquisitionDiagnostic>   (advisory only)
```

Frontend sees these; never sees tokens, license blobs, decrypt keys, device blobs, or raw provider responses.

### External Dependencies (Rust crates to add)

- `keyring` — macOS Keychain
- `prost` + protobuf descriptors for Widevine LicenseRequest/Response (minimal; only the message types Audible's flow uses)
- `aes`, `ctr`, `cbc` (RustCrypto) — decrypt cipher modes
- `rsa`, `ring` or `sha2` — request signing (RSA-SHA256 against `device_private_key`)
- `tiny_http` or built-in `tauri::http` — loopback HTTP listener for auth callback
- Existing already: `reqwest`, `tokio`, `serde`, `serde_json`, `uuid`, `chrono`

### Tauri Plugin Surfaces

- `@tauri-apps/plugin-opener` — already available through `tauriClient.openUrl`
  for HTTPS external-browser auth; local file preview/opening uses
  `tauriClient.openPath`.
- Optional fallback: tauri webview window for embedded login if loopback proves unreliable.

## Idempotence And Recovery

- **Auth**: token refresh is idempotent on `refresh_token`; full re-login replaces existing creds atomically. Device-registration is repeatable; old registrations get cleaned up server-side on logout.
- **Library scan**: re-runnable; cache survives crash; pagination uses Audible cursor.
- **Acquisition**: job ledger persists across restarts. Resume guidance: download phase is HTTP-Range-resumable; decrypt phase is streaming — partial decrypt requires re-download (acceptable for v1; revisit if titles are large enough that this matters). Cancellation cleans up partial staging files. `.partial` suffix lets a cleaner on startup remove abandoned writes.
- **Commit**: atomic rename; revalidate path at commit boundary; idempotent — re-running commit on an already-committed asset is a no-op.
- **Logout**: idempotent purge. Partial purge (e.g., network failure during server-side deregistration) leaves the local side fully purged and the server-side state staled-but-not-corrupted.
- **Widevine device identity**: cache is reusable across Android-registered accounts and independent of any single Audible account. Cache invalidation is explicit on parse/challenge failure; refresh attempts record source URL + hash. User-supplied override is idempotent and replaces resolver-provided cache atomically.

## Surprises And Discoveries

*(populated as work proceeds; initial known facts:)*

- **2026-05-16**: Three test files acquired via Libation (Billy Fingers / Star Trek / 7 Habits, source dates 2014 / 2019 / 1989) all report `AUDIBLE_DRM_TYPE: Widevine` in their `org.libation` Apple-list freeform tags. AAX legacy is effectively absent from active Audible distribution for this account. v1 ships without AAX-classic support.
- **2026-05-16**: AAXClean is GPL-3, not MIT. There is no easy license-clean decrypt library to vendor. Decrypt math must be derived from public protocol (FFmpeg AAX demuxer, public AAXC voucher field shapes, CENC/AES-CTR ISO spec, Widevine LicenseRequest protobuf shape).
- **2026-05-16**: Libation's auth shape is Amazon LWA via embedded browser, catching `/ap/maplanding` redirect. Standard OAuth-style redirect catch, NOT RFC 8252 PKCE. The complexity is in device registration + request signing, not the OAuth dance itself.
- **2026-05-16**: Libation handles Widevine device identity by caching a base64 CDM/WVD blob in `AccountsSettings.Cdm`; if missing/invalid, it fetches a URI list from its repo `.cdmurls.json`, tries resolver endpoints using a signed Android-account Audible API proof, validates the returned binary as a Widevine device, then caches it. ABB adopts this mechanism category (validated resolver + cache + override), not Libation's exact infrastructure or source code.

## Accepted Decisions

- **Native acquisition, not sidecar.** Sidecar contradicts the all-in-one product premise and routes Audible drift through someone else's release cadence without buying durability.
- **Widevine support is v1 scope, not deferred.** Evidence: Audible serves Widevine for active accounts even on legacy content.
- **AAXC is in scope opportunistically** if the API client + license response classification path is already built; the additional code is small.
- **AAX legacy is out of scope** until a real user title hits the unsupported path. Then it becomes a single-PR addition.
- **No metadata-enrichment seam.** Existing embedded tags + existing `search_online_metadata` lookup cover the metadata need. YAGNI.
- **No shared `JobRegistry`.** Acquisition lifecycle gets a peer `AcquisitionJobRegistry`. Processing's registry stays single-purpose.
- **Eighth Public API.** `RemoteSourceRuntime` would join the existing seven.
- **`LocalImportBridge` extraction** is a prerequisite for the work, not a follow-up. Done in Phase A.
- **GPL contamination rule** is hard-binding on agents implementing the Audible provider cluster.
- **Source acquisition format is a provenance label**, not a DRM-state label. Display "Source: Audible Widevine (DASH)" not "DRM: Widevine."
- **Widevine device identity policy**: resolver-based acquisition with local validated cache and user-supplied override. ABB does not vendor a Widevine device blob and does not treat Libation's `.cdmurls.json` as an owned dependency.

### Open Implementation Decisions (resolved in this spec when work hits them)

- **B2**: Loopback HTTP listener vs. embedded webview vs. external browser with user-paste fallback for the OAuth callback. Recommended: loopback first, embedded webview as graceful fallback. Decide during B2 implementation based on real-traffic observation.

## Validation And Acceptance

### Required to Claim Done

- `bun scripts/proof/runner.ts review` green.
- `scripts/check-no-remote-source-reach-through.sh` green (added in A4).
- `cargo test` in `src-tauri/` passes including new contract tests for `RemoteSourceRuntime` Public API Strip.
- Fixture-driven provider tests pass against committed fixtures under `src-tauri/tests/fixtures/audible/` covering: library response, AAXC license response, Widevine license response, MPD manifest, at least one failure-class fixture per acquisition phase.
- Widevine device-provider tests pass against fixtures for valid WVD bytes, invalid/corrupt cache, resolver failure, override import, and cache metadata recording. Tests must not call Libation's resolver or any live resolver in CI.
- Real-account smoke procedure documented + recorded in this spec's Progress section: at minimum one Widevine title successfully acquired, decrypted, committed, imported into the file list, and visible with "Source: Audible Widevine (DASH)" in the inspector. AAXC smoke optional, run if a title is reachable.
- Boundary-assertion test: an attempted reach-through import from outside `RemoteSourceRuntime` into the Audible provider cluster fails CI.
- Logout test: a logged-in account's purge leaves zero Vault entries, zero staging files, zero ledger entries for that account; library cache emptied; in-flight job (if present) cancelled cleanly.
- `docs/system-map.md`, `docs/api-map.md`, `docs/ubiquitous-language.md`, `src-tauri/src/remote_source/AGENTS.md` reflect committed reality.
- No `unsafe` Rust added outside any pre-existing FFmpeg/FFI boundary.

### Review-Agent Requirements

- UI proof: external browser-agent or human review of the acquisition flow including login, library, acquisition, import-ready handoff, and inspector "Source" field. Static assertions cannot prove this surface.
- GPL contamination review: spot-check authored provider code (D-phase Rust) for distinctive code patterns matching Libation source. Agents are expected to disclose if they referenced GPL source during implementation.

### Documentation Alignment

- `docs/system-map.md`: eighth Public API row.
- `docs/api-map.md`: Remote Source command + event family.
- `docs/ubiquitous-language.md`: new terms (Remote Acquisition Plane, Materialized Asset, Acquisition Job Ledger, Vault Adapter, Provider Driver, Source Acquisition Format, Capability Matrix).
- `src-tauri/src/remote_source/AGENTS.md`: Public API Strip, Private Cluster, allowed edits, breaking-change triggers, GPL rule.
- `docs/fallbacks.md`: register any compat paths emerged during D5/D6 with explicit triggers/signals/sunsets.

## Completion And Cleanup

This spec is deleted or distilled after:

- All Phase A–F work merged.
- Validation and acceptance gates green.
- Documentation alignment complete.
- `src-tauri/src/remote_source/AGENTS.md` published as the durable home for Remote Source invariants.
- No outstanding follow-ups belong in this spec — open follow-ups become GitHub issues (e.g., "add AAX-legacy support when first user hits it," "consider per-segment DASH download for very large titles").

## Progress

Add timestamped progress notes here as work proceeds.

- **2026-05-16** — Spec created. Decision-alignment loop with jstar confirmed: native acquisition, Widevine v1, no metadata seam, AAX-legacy deferred, GPL contamination rule for agents, and a new Remote Source Public API shape.
- **2026-05-16** — Widevine device identity policy locked: ABB uses a private resolver-backed `WidevineDeviceProvider` with validated local cache and user override, but does not vendor a device blob or depend on Libation's `.cdmurls.json` as owned infrastructure.
