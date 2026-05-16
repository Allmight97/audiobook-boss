# Remote Acquisition — Decision-Alignment Chat Record

This document preserves the Cursor IDE chat that produced
`docs/specs/remote-acquisition.md`. Captured on 2026-05-16 at the repo
owner's request after spec creation, so future agents resuming this
work can see how the design choices were made, what alternatives were
considered and rejected, and what evidence anchored each pivot.

Conventions:

- `## User — Turn N` blocks contain the repo owner's (jstar's) messages verbatim, including original spelling and phrasing.
- `## Assistant — Turn N` blocks contain my replies verbatim, with internal reasoning traces omitted.
- Italic notes inside assistant turns mark material tool activity (file reads, shell commands, subagent dispatches). Tool output excerpts that were quoted or cited in the reply are preserved in code blocks; full raw output is not included unless directly load-bearing.
- Horizontal rules separate turns.

The transcript ends with the request that produced this record. No assistant body is included for the final turn (it would be this file).

---

## User — Turn 1

```
Here's a design concept for a set of features: /Users/jstar/Documents/Codex/artifacts/audiobook-boss/2026-05-15-libation-inspired-architecture-options.html

Goal: We're designing a first-class Remote Acquisition capability for Audiobook Boss, starting with Audible as the first provider. The feature should authenticate a user securely, discover selected Audible titles, acquire/decrypt AAX/AAXC/DASH assets into verified local M4B files, and then hand those files into ABB's existing manual import/inspect/process workflow. The core design question is how to make this bold enough to be a real product capability without letting Audible auth, DRM, library-management scope, or provider volatility leak into ABB's existing processing core.

Is the proposed Remote Acquisition Plane -> MaterializedAsset -> existing ABB import boundary the right architectural shape?
Are the grey-box modules deep enough, or are we still leaving too much coordination in UI/IPC glue?
Are the proposed forbidden paths correct: no tokens to frontend, no DRM internals to processing, no auto-processing?
Is native Rust acquisition the right target architecture, or should sidecar/tool integration be treated as more central?
What implementation risks would you force us to resolve before committing to this direction?
---
And stated otherwise: This concept is inspired by the Libation app a clone of which is here /Users/jstar/Projects/tmp_repos along with AAX clean clone. Libation app already proved this can be done reliably and my goal isn't to clone libation nor turn ABB into library management. The goal is for ABB to enhance his aquistion ability to extend it beyond manually loading files per current workflow while eliminating the need for the user to go outside ABB to aquire titles from their Audible account. Aspects of the design (not yet sure which elements to refer to it by) that handle authentication should be maintainable and durable yte flexible to support authentication to other sources than auidble accounts.

The user outcome at a high-level: I want useres to be able to use ABB as the "all in one" app to process audiobooks and metadata which includes minimal features needed to capture audiobooks from audible (and other places), decrypt them, and load them into the exsting ABB processing architecture.
```

---

## Assistant — Turn 1

*Read the design HTML, the decision-alignment skill (`.codex/skills/decision-alignment/SKILL.md`), `docs/system-map.md`, `docs/ubiquitous-language.md`, `docs/fallbacks.md`, `src/ui/fileImport/handlers.ts`, `src-tauri/src/audio/path_validation.rs`, `src-tauri/src/processing/types.rs`, `src-tauri/src/commands/metadata_lookup/providers/audible.rs`, and `src-tauri/src/ipc_contract.rs` to ground the critique in actual repo state. Confirmed Libation/AAXClean clones exist under `/Users/jstar/Projects/tmp_repos/`.*

Before answering, I cross-checked the design doc's repo claims against actual code so I'm critiquing what ABB really is, not what the doc says it is.

**Verified accurate in the artifact**:
- `validate_input_audio_path` rejects symlinks, validates regular-file + extension, canonicalizes (`src-tauri/src/audio/path_validation.rs:12-35`)
- `ProcessPayload.input_files: Vec<String>` is the processing contract — local paths only, no remote shape today (`src-tauri/src/processing/types.rs:19-33`)
- Metadata-lookup Audible provider is unauthenticated catalog search only (`src-tauri/src/commands/metadata_lookup/providers/audible.rs`)
- `processFilePaths` is a private helper at the bottom of `handlers.ts`, exactly the seam the design wants to extract (`src/ui/fileImport/handlers.ts:153-171`)
- IPC contract registration is centralized in `ipc_contract.rs` and Specta-driven; no generic command exists today

**Worth flagging**: `docs/fallbacks.md` is currently nearly empty (only `FB-018`). That means the boundary-assertion + fallback-register discipline the design assumes is healthy is *barely exercised* in ABB today, which raises the stakes on whether a new grey-box module of this size actually gets the boundary checks it needs (more on this in risks).

Now to your five questions.

---

### Q1. Is `Remote Acquisition Plane → MaterializedAsset → existing ABB import` the right shape?

**Yes on the spine, with one substantive correction on what `MaterializedAsset` is allowed to carry.**

The shape is essentially hexagonal architecture done right for ABB's grey-box discipline: provider as a port, MaterializedAsset as an inbound primary port, and ABB's existing manual workflow as the consuming use case. It preserves the system-map's boundary rule ("UI expresses intent, Rust produces durable truth") and keeps Audible chaos quarantined.

**The correction**: the artifact silently makes MaterializedAsset carry three different categories of truth:

1. **Verified local path** (artifact truth) — clearly belongs at this boundary.
2. **Provenance manifest** (source-of-record audit) — also clearly belongs.
3. **"Suggested metadata, chapters, cover observations"** — this one is contraband if it crosses the gate the way the artifact implies.

(3) is the seam to watch. ABB already has a `Metadata Intent Plan` grey-box Public API with strict `set`/`clear`/`noop` semantics (canon: `docs/ubiquitous-language.md`, "Metadata Intent Patch"). If the acquisition layer dumps Audible's rich metadata directly into the file's metadata draft, you create an **Ownership Smear**: two modules both partially own "what is the metadata intent for this file?" — Remote Acquisition for fresh imports, the existing metadata pipeline for everything else.

**Sharper shape**: split the green-gate output into two distinct artifacts.

- `MaterializedAsset` = path + container facts (`ffprobe`-equivalent) + provenance. This is what the import bridge accepts. Treated identically to a picker/drop file.
- `RemoteMetadataObservation` = optional sidecar, surfaced through the **existing** `search_online_metadata` / metadata-lookup pipeline as a high-confidence candidate source (provider = `audible.account`, ASIN, narrators, series, chapter list, cover URL). The user accepts it through the existing draft UI; intent semantics stay in their owning module.

This change is small in code but big in ownership clarity: it means the Audible provider's rich knowledge survives the gate **without** acquisition becoming a second metadata-intent authority. It's also the seam that makes "Audible chapter import is great" not require special-casing — it just becomes a stronger result in the lookup system you already have.

---

### Q2. Are the grey-box modules deep enough?

**Audible provider: yes.** AAX/AAXC/DASH/ADRM/license/marketplace inside one private cluster is exactly the Ousterhout shape — small public capability strip, substantial hidden mechanism.

**Remote Acquisition Runtime: at risk of being shallow.** The proposed Public API is four runtime types:

```text
RemoteSourceRuntime, RemoteLibraryRuntime, RemoteAcquisitionRuntime, ImportBridge
```

That's 12+ methods across four façades. ABB's existing Five Public APIs are *each* one runtime: `tauriClient`, `processing::plan`, output artifact plan/commit, metadata intent plan, status panel. Splitting RAP into four runtimes mirrors *internal organization*, not *caller categories*, and is the classic "façade decomposition" anti-pattern the system map already warns against ("Do not use this bias to create generic managers, controllers, routers, or facades that merely rename complexity").

**Recommendation**: collapse to two grey-box modules.

| Module | Public API Strip | Private Cluster |
| --- | --- | --- |
| `RemoteSourceRuntime` | `providers()`, `accountStatus(providerId)`, `beginAuth/completeAuth/logout`, `scanLibrary`, `preflightAcquisition`, `startAcquisition`, `cancelAcquisition`, `acquisitionStatus`, `commitMaterializedAssets` | provider registry, account refs, vault adapter, library cache, acquisition job ledger, staging root, phase progress, provenance writer, capability matrix |
| `LocalImportBridge` | one entry: `importLocalAudioFiles(paths)` returning `FileListInfo` (the extraction of `processFilePaths`) | analysis, order-lock check, draft staging, FileListInfo append |

Two runtimes, each with a real depth-to-surface ratio. `ImportBridge` is also reused by picker/drop, which is what enforces the "remote import is not a privileged bypass" invariant.

**Depth test for RemoteSourceRuntime to pass before commit**: can a caller go auth → library → acquire → commit → ready-for-import without the UI doing any state choreography? If the UI has to sequence "after auth, refresh library; after acquisition, then commit," lifecycle is leaking out and the module isn't deep enough yet. The UI should be subscribing to phase events and rendering, not orchestrating.

**Also missing from the depth analysis**: where does `Vault` live? The artifact correctly hides it inside the runtime, but doesn't propose the corresponding **Boundary Assertion** (a `scripts/check-no-bridge-imports.sh`-style script that blocks reach-through imports). Without that, the next agent who needs to "just grab a token for logging" reintroduces the leak. If you commit to this shape, the boundary assertion has to land in the same PR family as the public API strip.

---

### Q3. Are the forbidden paths correct?

**Correct on the three stated, but the list is incomplete.**

The three you have ("no tokens to frontend / no DRM internals to processing / no auto-processing") are right and well-stated. The "auto-processing" framing needs a small sharpening, and three more belong on the list:

1. **Sharpen "no auto-processing"** to "**Acquisition never causes processing.**" The forbidden path is causality, not user policy. Users may legitimately want a setting "after Audible acquisition, auto-process with my default profile." That's fine — as long as it lives in **processing settings**, the user explicitly opted in, and the trigger crosses the green gate as a normal local-file import that happens to be auto-handed-to processing. Acquisition stays ignorant of it.

2. **Add**: **No provider-supplied output paths cross the gate.** Provenance manifests are advisory. The staging root, generated filename, and final committed path are all ABB-owned. Provider-controlled filenames are a path-traversal attack class (`../`, NUL injection, extension lying). This is implicit in your "owned staging root + atomic commit" but should be elevated to a top-level forbidden-path because it directly maps to a CI-checkable rule: provider modules must not write into any path outside the staging root they were given.

3. **Add**: **No provider metadata enters `Metadata Intent Plan` without an explicit user accept.** Per Q1 — provider observations are candidates in the lookup pipeline, never direct writes to the draft. Prevents Ownership Smear.

4. **Add**: **No zombie provider state survives logout.** Atomic on logout: keychain entry deletion, library cache deletion, in-flight acquisition cancellation+cleanup, job-ledger entries for that account removed, staging files for that account purged. Plus: deregister the device upstream when the protocol supports it. Audible has per-account device limits; orphaned device registrations are a real Libation pain point.

These four together are CI-testable claims, which is the bar this design should aim for — boundaries aren't real until they fail a check when violated.

---

### Q4. Native Rust vs sidecar — right target?

**Native Rust is the right target *shape*. Sidecar is probably the right *first build*.** This isn't a retreat from A+; it's a sequencing decision the artifact's "Recommended/Fallback" framing obscures.

Three uncomfortable facts the artifact glosses:

**(a) "External browser auth via opener" doesn't map cleanly to Audible.** RFC 8252 + PKCE assumes the provider supports a public OAuth client with a registered redirect. Audible doesn't. Libation, AudibleApi, and audible-cli all use Amazon's *web login flow* — captured cookies, device-registration endpoints, derived identity tokens. The actual mechanism is either:

- Embedded webview that captures Amazon login (Libation; UX is clear but contradicts the external-browser principle the artifact builds on)
- Manual cookie/auth-data paste (audible-cli; degraded UX)
- Unofficial device-registration endpoints driven by code (Libation/AudibleApi; drift-prone)

There is no clean RFC 8252 path for Audible today. The artifact's auth picture (PKCE + loopback + keychain) is *architecturally* correct as the durable shape but **the auth mechanism that fills the shape is provider-private**. The Vault Adapter pattern stays, the keyring storage stays, but the front-of-Vault is `Amazon login + device registration`, not OAuth/PKCE. The doc should be honest about this — the difference between "external browser launches OAuth" and "embedded webview captures Amazon session" is the difference between a 1-week implementation and a 3-month one.

**(b) Decrypt feasibility splits by format.** AAX (legacy, activation-bytes-based) has multiple open Rust implementations (`inAudible-NG`/`aaxtomp3` ecosystem) and is plausible clean-room. AAXC (per-title voucher keys) is harder but tractable. AAX/DASH/Widevine (the direction Audible is *actually* pushing) is the wall — Widevine CDM is closed; Libation works around with specific provider behaviors. **Native Rust will be feature-incomplete vs sidecar for years on newer content.**

**(c) The actual maintenance cost.** Libation is a multi-year continuous maintenance burden by one developer with deep Amazon-protocol knowledge. ABB is "one person + agents" with audiobook-processing as its value prop. Adopting a maintained unofficial Audible client as a *core ABB competency* is a strategic decision, not just an architectural one.

**The shape that actually de-risks this**:

- Lock the **provider driver trait** native-first as the durable contract (matches A+).
- Ship **`audible.sidecar`** as the first concrete driver (matches B but treated as the implementation, not the fallback). Concretely: shells out to Libation or audible-cli, reads their output manifest, atomically copies into ABB-owned staging, runs ABB path validation, commits MaterializedAsset.
- Build **`audible.native`** behind the same trait when (and only when) a clean-room implementation is feasibly scoped — start with AAX legacy where the protocol is well-understood, add AAXC later, accept that Widevine content stays on the sidecar path.
- **`audible.external`** (folder-watch / manual import) stays as the lowest-friction onboarding and as the contract conformance baseline that every driver must match.

This preserves everything that matters in A+: grey-box module shape, MaterializedAsset boundary, vault adapter, staging, provenance, capability matrix, job lifecycle, contract tests. It just changes which provider implementation ships first.

**Hard durability test**: a user switches from `audible.sidecar` to `audible.native` and (i) their auth survives, (ii) their library cache survives, (iii) their committed M4Bs and provenance survive, (iv) no UI changes. If yes, the trait is right. If no, the trait isn't actually provider-neutral.

**A subtle correction on Option C (.NET worker)**: the artifact rejects it on GPL grounds. Worth knowing precisely: AAXClean is MIT-licensed; Libation is GPL-3. A .NET worker built *only* on AAXClean (without Libation's account/library code) is MIT and ships without GPL obligations. The reason to still reject C isn't license — it's distribution complexity (signed .NET runtime, notarization, updater, IPC contract surface) that ABB doesn't otherwise need. Worth saying "rejected for distribution burden, not license" so the door stays open if priorities change.

---

### Q5. What implementation risks must be resolved before committing?

The artifact names some of these; these are the ones I'd hard-block on before any code lands.

**R1. Auth mechanism honesty (highest leverage).** Per Q4(a): pick one of `embedded-webview-captures-Amazon-session`, `manual-auth-paste`, or `code-driven-device-registration` *now*. Each implies different UI, different keychain payload shape, different revocation semantics, different threat model. The current artifact reads like "RFC 8252 will save us"; it won't. **This is the single decision that determines whether native is even on the table for v1.**

**R2. License-decryption capability matrix is incomplete.** Decide which content classes ABB Audible support means. Concretely: AAX only? AAX + AAXC? Anything Widevine-protected? If the answer is "everything Libation supports," that's a sidecar commitment, full stop. If "AAX + AAXC only," native is sketchable. The capability matrix needs to be ABB-product policy, not a side-effect of provider drift.

**R3. Job lifecycle ownership: peer registry, not shared.** Acquisition's phases (libraryScan, license, download, decrypt, verify, stage, commit) don't map onto ABB's existing `JobRegistry` (which owns processing concurrency with encoder/path/preflight). Reusing it creates an Ownership Smear: two job-lifecycle definitions, one registry. Build a peer `AcquisitionJobRegistry` that mirrors the *shape* of the processing one (terminal outcomes: success / cancelled / failed; per-job state; cancellation discipline) but doesn't physically share it. The `Status Panel Runtime` Public API can render both because it already takes truthful backend status — that's the right shared surface.

**R4. Provider metadata seam decision (per Q1).** Decide before commit: does `audible.*` produce a `RemoteMetadataObservation` consumed by the existing metadata-lookup pipeline, or does the acquisition runtime write into the draft? My strong recommendation is the former. Without this decision, two grey-box modules will both partially own metadata intent.

**R5. Fixture corpus + drift detection that doesn't require a live account.** "Drift fixture lab" is named in the artifact but unstaffed. Concretely: sanitized library response JSON, license voucher JSON, DASH manifest XML, ADRM challenge/response, and at least one fail-case fixture per provider phase. Without this, the Audible provider's contract tests effectively only run on machines with active accounts, which means CI doesn't catch drift, which means provider regressions land in user installs.

**R6. GPL contamination guard for agents.** The Libation/AAXClean clones in `~/Projects/tmp_repos` are literally a few directories from this conversation. Without a hard rule, the next agent implementing `providers/audible/license.rs` reads `Libation/AaxDecrypter/*.cs` for "reference," which is a real GPL contamination risk for the Rust output. Practical guard: a `.cursorignore`/equivalent + an `AGENTS.md` rule in `src-tauri/src/remote_source/providers/audible/` that says "implementation may use protocol docs, behavior tests, and public network captures only; reading GPL-derived source files is prohibited for agents and humans working on this cluster." This is a one-line policy that prevents a project-ending license outcome.

**R7. Logout completeness as a lifecycle (per Q3 #4).** Atomic. Including upstream device deregistration when supported. Including in-flight acquisition cancel-and-cleanup. Including journaled cleanup if the app dies mid-logout. Without this, multi-account use creates orphaned device registrations and stale staging files.

**R8. Concurrency policy declared, not assumed.** Default to 1 in-flight acquisition per account, configurable cap on total. Acquisition is rate-limit-sensitive (Audible's license API) and bandwidth-sensitive in a way ABB's encoder concurrency tuning doesn't address. Reusing `get_max_concurrent_jobs` would be wrong; it owns encoder concurrency.

**R9. ImportBridge call site identity.** The artifact proposes `analyzeMaterializedAssets(assetIds) -> FileListInfo` as a new IPC command. The existing seam is `analyzeAudioFiles(paths: string[]) -> FileListInfo`. If MaterializedAsset is "verified local path + provenance," there's no analytical difference at the audio layer — provenance is metadata-side. The simpler shape is: `LocalImportBridge.importLocalAudioFiles(paths)` is the **only** import entry point, and the remote source UI calls it with committed-asset paths exactly the way picker/drop does. The provenance hangs off a separate side table indexed by canonical path. This keeps "one local-media ingress path" literal at the IPC level too, not just at the architectural-diagram level.

**R10. `docs/fallbacks.md` discipline must be primed before this work.** The register currently has one entry. Remote acquisition will produce real fallbacks (e.g., "AAXC voucher fetch fails → fall back to sidecar for that one title," or "Widevine-protected content → require sidecar"). These need register entries with triggers, signals, and sunsets — otherwise they become silent compat shims, which is exactly what ABB's design bias forbids. The register's discipline being *barely used today* is a project-health risk for accepting a feature that will heavily use it.

---

### Synthesis

**The bones of the proposal are right.** Remote Acquisition Plane → MaterializedAsset → existing import is a sound shape that protects ABB's processing core, fits the grey-box discipline, and matches the system-map's boundary rule. It's the most architecturally honest framing I've seen for this product direction.

**The body needs three changes before commit**:

1. Split the green-gate output: `MaterializedAsset` (path + provenance) goes to import; `RemoteMetadataObservation` goes to the existing metadata-lookup pipeline. No new metadata-intent authority.
2. Collapse the four runtimes into two grey-box modules: `RemoteSourceRuntime` and `LocalImportBridge`. Add the boundary-assertion script that blocks Vault reach-through.
3. Sequence native-after-sidecar with the same trait. Frame B as "first build," not "fallback." Make the trait-swap test (`sidecar → native` survives auth, cache, M4Bs, no UI change) a commit gate.

**The proposal owes more honesty on Audible auth specifically (R1) and the capability matrix (R2)** — these are the two decisions where "we'll figure it out in implementation" will eat 80% of the project's time.

The result is something ABB can actually own as a durable product capability rather than a Libation maintenance project disguised as a feature.

---

### Highest-leverage open questions (decision-alignment routing)

**Q-A. Provider sequencing: sidecar-first or native-first?**
Recommended: **sidecar-first (`audible.sidecar` ships v1 behind the native-shaped trait), native (`audible.native`) targeted for AAX legacy after the trait is proven by sidecar.** This preserves the A+ target architecture while making v1 actually shippable and avoids committing ABB to multi-year Amazon-protocol maintenance as a core competency before you've proved users want this enough.

**Q-B. Metadata seam: does acquisition produce `RemoteMetadataObservation` consumed by the existing lookup pipeline, or write directly to the metadata draft?**
Recommended: **observation consumed by the lookup pipeline.** Keeps `Metadata Intent Plan` as the single owner of set/clear/noop semantics. Provider gets a new high-confidence source row; user still drives the draft.

Once those two are answered, I'd recommend routing the accepted shape into an active `docs/specs/remote-acquisition.md` via the decision-alignment Spec Handoff — the work is multi-session, multi-subsystem, and has real verification gates (boundary assertions, contract tests, fixture corpus, GPL guard), exactly the kind of thing the spec format exists for. I won't create that file unless you greenlight it.

Want to switch to plan mode and iterate on Q-A and Q-B before any code or spec touches the repo?

---

## User — Turn 2

```
"Native Rust is the right target shape. Sidecar is probably the right first build" - as I said to the agent that helped buiold that design: 

"And I agree to need to avoid ABB becoming a libation clone. The only aspects of libation I wish to take inspiration from is the outcomes it basicaly serves of aquiring and decrypting aax files into m4b files we can then pass through the normal ABB processing flow. You bots are entirely too safe always talking about "first build" and starting small and increments."

I unddrstrand that need to control complexity, but humans aren't writing code like they did and so we have shifted to taking more time in design, architecture, and laying boundaries for agents to generate the code and go from there. I've no patience for safe and gradual solutions unless there's real material risk or blockers they are working to address.

Also, some of your feedback might change if you simply looked at how libation is authenicating, aquiring, and decrypting. Yes it's a different codebase, but the concept we should be able to extract and translate into ABB.

Based upon your sythnesis it sounds like we are on the same page about the outcomes and shape. My goal here isn't to refine the HTML content on that page, the goal was to simply share it with you and help narrow things down so we can start implementing.

A. Side car rejected because it sounds like "officer safety" is opining here. I don't see the value of doing this. I'm lioterally the only human dev and user on this project. Be serious.

B. I'm not sure now to respond here. LIbation downloads and decrypts the audible file into an m4b without (as far as I can tell) mutating metadata already embedded in the source. ABB shouldn't be doing more with metadata than "passing it through" since we adding an adiditonal aquisitiion pathway. What am I misunderstanding here? Auth > DOwnload > decrypt > decryted m4b with embedded metadata from source lands in existing ABB input handling > user massages metadata and audio processing settings > processes file.
```

---

## Assistant — Turn 2

*Dispatched an `explore` subagent (readonly) to map Libation's auth, library, license, download, decrypt, and metadata behavior with file:line citations from `/Users/jstar/Projects/tmp_repos/Libation` and `/Users/jstar/Projects/tmp_repos/AAXClean`. Key findings used inline in the reply below; full subagent report not duplicated here. Material facts surfaced:*

- *Auth: Libation delegates to the `AudibleApi` NuGet (not vendored); user-facing flow is an embedded WebView (WinForms `WebLoginDialog.cs:73-83`, Avalonia `AvaloniaLoginChoiceEager.cs:88-95`) that catches Amazon `/ap/maplanding` redirect.*
- *Persisted identity is rich: `adp_token`, `access_token`, `refresh_token`, `device_private_key`, `store_authentication_cookie`, `device_info`, `customer_info`, `expires`, `locale_code`, `activation_bytes`, plus base64 CDM device blob (`Mkb79Auth.cs:17-51`, `AccountsSettings.cs:51-64`).*
- *DRM classification by license-response key shape (`FileLiberator/DownloadOptions.cs:62-74`): 4-byte key → AAX, 16+16 key+IV → AAXC, `DrmType.Widevine` → DASH/CENC.*
- *Decrypt: AAXClean's `AaxFile.cs:76-118` (AAX SHA1 chain → AES-CBC keys), `DashFile.cs:43-118` + `DashFilter.cs:17-25` (CENC AES-CTR streaming).*
- *Metadata: `AllowLibationFixup` (default ON) actively overwrites tags from Audible API at `DownloadDecryptBook.cs:209-273`; chapters rebuilt via `Api.GetContentMetadataAsync`.*
- *Licenses: AAXClean is **GPL-3** (not MIT as I previously claimed). Libation is GPL-3.*
- *Codebase weight: AaxDecrypter ~1474 LOC, FileLiberator ~1940 LOC, AudibleUtilities ~1168 LOC excluding ~14.5k generated Widevine protobuf.*

Fair pushback on both. I went into "officer safety" reflex on A and over-architected B. Reading Libation's actual code changes the picture in concrete ways. Let me recalibrate.

### First, three factual corrections I owe you

These change what I said before:

1. **AAXClean is GPL-3, not MIT.** I had that wrong. The .NET-worker option (Design C) had no GPL escape hatch — my footnote was misleading. There is no easy ready-made decryption library you can adopt without GPL contagion; the decrypt math has to be reimplemented from public protocol knowledge regardless of path.

2. **"RFC 8252 doesn't fit" was an overstatement.** Libation's auth is actually a normal OAuth-style redirect catch — an embedded browser navigates Amazon LWA pages, and `/ap/maplanding` is the success-redirect signal (`Source/LibationWinForms/Dialogs/Login/WebLoginDialog.cs:73-83`, mirrored in Avalonia at `Source/LibationAvalonia/Dialogs/Login/AvaloniaLoginChoiceEager.cs:88-95`). MFA is handled organically because the user does it in the WebView. The "no public OAuth client" caveat stands — you're masquerading as an Audible-app device, not a registered third-party OAuth app — but the *shape* (open browser, catch redirect URL, extract tokens) is standard and not exotic.

3. **Libation does NOT do strict metadata passthrough.** This is the key correction for your question B (see below).

### A. Native-first, by format phase. Sidecar dropped, not deferred.

You were right. My sidecar framing was buying time, not buying durability. Audible API drift is the same risk whether Libation absorbs it or you do — sidecar just routes the breakage through someone else's release cadence and adds install friction that contradicts the "all-in-one" product premise.

What the Libation read actually shows is that the decrypt and acquisition mechanics are **bounded and well-understood**, not arcane. The volatility is concentrated in two specific places: auth + license shape, and Widevine key extraction. Everything else is mechanical translation.

**The sequencing that respects "no patience for safe and gradual" while still being honest about a real boundary**:

| Phase | Scope | Decrypt math | License surface | Open implementation precedent |
| --- | --- | --- | --- | --- |
| **P1: AAX (legacy)** | Older Audible catalog, activation-bytes era | AES-CBC, key derived from `adrm` box SHA1 chain (documented in FFmpeg, AAXClean confirms shape at `src/AAXClean/AaxFile.cs:76-118`) | `Api.GetDownloadLicenseAsync` returns activation bytes; single-file HTTPS Range download | FFmpeg AAX demuxer, `aaxtomp3`, `inAudible-NG` projects — math fully public |
| **P2: AAXC** | Modern non-Widevine catalog | AES-CBC with explicit 16-byte key + IV from voucher (`Source/FileLiberator/DownloadOptions.cs:62-74` classifies `Adrm + 16-byte key + 16-byte IV → FileType.Aaxc`) | Voucher response carries `KeyPart1` (key) + `KeyPart2` (IV) directly | Open precedent in audible-cli, AAXClean |
| **P3: DASH/Widevine** | Newest Audible catalog Audible is actively pushing toward | AES-CTR with content keys *extracted from a Widevine license response* — Libation isn't running a Widevine playback CDM, it's parsing the license blob to recover AES keys for plain CTR decryption | Requires Android-shaped device registration + a serialized Widevine "device" blob (`Source/AudibleUtilities/Widevine/Cdm.Api.cs:24-52`, with the blob sourced from a community-hosted `.cdmurls.json` at `Cdm.Api.cs:99-101`) | This is the only part with no clean open precedent and ongoing legal sensitivity |

P1+P2 cover a meaningful slice of any user's actual Audible library (anything that hasn't been re-mastered into DASH) and use entirely open math. P3 is the real architectural boundary — not because it's gradual or cautious, but because it's where ABB has to consciously decide whether it owns a Widevine-key-extraction pathway with a community-sourced device blob. That's a product decision, not an engineering decision.

**These three are not "phases of caution."** They are three different content-protection regimes that Libation itself selects between at runtime (`DownloadOptions.cs:62-74` is exactly the decision tree: 4-byte key → AAX, 16+16 key/iv → AAXC, Widevine DrmType → DASH). Shipping P1+P2 as v1 is a complete native product; P3 is its own commit with its own conversation.

**Auth scope, concretely** (this is what `EzApiCreator` / `AudibleApi` actually does behind Libation's UI):
- Open external/embedded browser to Amazon LWA login URL constructed for the locale
- Catch `/ap/maplanding` redirect, extract authorization code from the URL
- POST to Amazon device-registration to exchange code → `adp_token` + `access_token` + `refresh_token` + `device_private_key` + `store_authentication_cookie` + `device_info` + `customer_info` (`Source/AudibleUtilities/Mkb79Auth.cs:17-51` enumerates the persisted shape)
- Persist to OS keychain (your design)
- Refresh via stored `refresh_token` when access token expires
- For Widevine-eligible accounts, also register an Android-shaped `DeviceType` so license requests are eligible

That's mechanical and bounded. The hard part isn't the OAuth dance — it's the device-registration request signing, which uses the Amazon-derived `device_private_key` to sign API requests with a specific header format. That's the surface Libation outsources to `AudibleApi` (NuGet, ~separately maintained, not vendored in the clone). For native Rust, that's the one piece you'd genuinely need to reverse-engineer from observed traffic. Estimated effort: real but bounded — a few hundred lines of Rust around `reqwest` + `rsa` + `ring`.

**Codebase weight for clean-room translation** (excluding the metadata draft seam below):
- AAX/AAXC decrypt math (P1+P2): translatable from AAXClean structure + FFmpeg AAX demuxer + public AAXClean comments. ~500-1000 LOC of Rust.
- HTTP Range download + resume: ~200 LOC.
- Audible API client (auth/library/license endpoints, request signing): ~800-1500 LOC, gated by reverse-engineering effort on `AudibleApi`'s device-signing.
- Job lifecycle + staging + provenance + commit: ~500-800 LOC.

So P1+P2 is on the order of 2-4K LOC of Rust. Not trivial, but not Libation-scale either, because we're skipping the entire library-management half (DataLayer, search, Hangover, Avalonia UI — about half of Libation's source).

### B. Metadata — you're partially right, partially missing one thing

Your mental model is reasonable and almost works. The thing it misses is that **Libation does not pass through embedded metadata by default**.

The path is:
1. Decrypt opens the AAX/AAXC and loads `AaxFile.MetadataItems` (the embedded source tags) — this is your "passthrough" baseline.
2. When `AllowLibationFixup = true` (the **default**), `DownloadDecryptBook.Converter_RetrievedMetadata` **actively overwrites** the M4B's tags from Audible API library data before the file is finalized: title from `LibraryBookDto.TitleWithSubtitle`, artist/album-artist/narrator from library DB, genre from category tree, publisher from API, release date from API, comment/description from API, cover art from the API's high-res URL via `SetCoverArt`, and chapters rebuilt from `Api.GetContentMetadataAsync` (with a known-bad-last-chapter padding fix Audible's chapter durations need) (`Source/FileLiberator/DownloadDecryptBook.cs:209-273`, chapter refinement at `Source/FileLiberator/DownloadOptions.Factory.cs:36-65`).

So Libation's output M4B is **API-curated metadata baked into the file**, not a faithful pass-through of the AAX-embedded tags.

This matters because the AAX-embedded metadata is often poor: low-res cover, sparse description, chapter durations that drift, sometimes wrong release dates for re-mastered titles, no series info, no narrator field on older books. Libation users *love* the fixup — it's most of the perceived value.

**Which means your three options are**:

1. **Strict passthrough** (your stated vision): acquisition decrypts and hands the M4B over. ABB analyzes embedded tags like any picker/drop file. Simple. Cost: you ship a worse product than Libation users are accustomed to, especially on older content. No new architectural seam needed.

2. **Bake-at-acquisition** (Libation's default): acquisition writes Audible API metadata into the M4B *before* commit. Cost: acquisition becomes a metadata-intent authority alongside ABB's existing draft system → two modules both partially own "what metadata is on this file." That's the smear I was flagging earlier.

3. **Surface as candidate** (what I was awkwardly proposing): acquisition produces the M4B with embedded passthrough metadata + a sidecar `RemoteMetadataObservation` blob (title/author/narrator/chapters/cover-URL/description/etc.). The observation lands in ABB's **existing** metadata-lookup pipeline (`search_online_metadata` and its draft surface) as a new account-scoped provider source. The user clicks "apply" in the existing draft UI if they want it. Acquisition writes nothing it isn't asked to write; the metadata-intent system stays the single authority.

(3) is what I should have explained more concretely. It's not a complex new architecture — it's literally "add a second provider to the same metadata-lookup surface that already searches Audible's public catalog, with the difference that this one is account-scoped and has license-grade accuracy (real chapters, high-res cover, exact ASIN/ACR identifiers)." The plumbing reuses what's already in `src-tauri/src/commands/metadata_lookup/` today.

You get Libation's product value, ABB doesn't grow a second metadata-intent authority, and your stated model — "M4B lands in normal ABB input handling" — stays literally true.

**My recommendation**: option 3. But option 1 (strict passthrough) is a legitimate product decision if you'd rather defer enrichment to a later release. It's not technically wrong, just leaves real value on the table.

### What changes in the design

Concretely, vs the artifact:

- Drop the "B. Sidecar" entry. Replace with "**Phase plan: P1 AAX → P2 AAXC → P3 DASH/Widevine**," each shipping as native Rust.
- Reword the auth section: external-browser auth catches Amazon LWA `/ap/maplanding` redirect; persist `adp_token`, `access_token`, `refresh_token`, `device_private_key`, plus device-registration response (`adp_token` + private key signs subsequent Audible API requests).
- Update the MaterializedAsset boundary: produces `MaterializedAsset` (path + container facts + provenance) **plus** an optional `RemoteMetadataObservation` consumed by the existing metadata-lookup pipeline. Acquisition writes no tags to the M4B itself beyond what decrypt naturally preserves.
- Promote Widevine/CDM blob handling out of the architectural design and into a **Phase 3 product-policy decision** with its own evidence and risk paragraph. P1+P2 is a complete v1.
- The GPL contamination rule for agents is still the right guardrail: implementation works from this behavioral map + protocol observation + tests, not from copying Libation source. (Behavioral maps describe protocol, which isn't copyrightable; copying file structure or distinctive code patterns is.)

### Next move

The decision-alignment routing is straightforward: the shape is agreed, the open questions are bounded, and the work is multi-session multi-subsystem with real verification gates. That's exactly a `docs/specs/<task>.md` situation.

Concretely, I'd like to write `docs/specs/remote-acquisition.md` with:

- Outcome statement and product scope (acquire+decrypt selected Audible titles into M4B files that enter the existing ABB import flow)
- Grey-box module shape: `RemoteSourceRuntime` (auth/library/acquisition lifecycle) + `LocalImportBridge` (shared local-file ingress)
- Public API strip for each, with the boundary assertions
- Phase plan: P1 (AAX) + P2 (AAXC) as v1; P3 (DASH/Widevine) as a separately-decided follow-on
- Auth mechanism specified at the level of "external browser catches `/ap/maplanding`, persists this exact token shape"
- `MaterializedAsset` + `RemoteMetadataObservation` contracts
- Forbidden paths (the four from my Q3) with corresponding boundary-assertion scripts
- Fixture corpus expectation (sanitized library/license/manifest fixtures stored in repo)
- GPL contamination rule for agents touching `src-tauri/src/remote_source/providers/audible/`
- Proof path: contract tests, boundary assertions, real-account integration smoke for each phase

Want me to draft that spec next? If yes, I'll do the read-and-write here. If you want to settle anything else first (especially metadata seam choice 1 vs 3, and whether P3 is in or out of scope for v1 thinking), tell me and I'll roll the answers into the draft.

---

## User — Turn 3

*Attached two screenshots from the ABB UI showing file inspector cards. First: "The Afterlife of Billy Fingers" — Bitrate 62794 kb/s, Sample Rate 22050 Hz, 2 ch, AAC-LC, Native AAC (FFmpeg) decoder, 135.8 MB. Second: "Star Trek: The Motion Picture" — 114616 kb/s, 44100 Hz, 2 ch, USAC / xHE-AAC, Apple AAC decoder, 329.1 MB.*

```
On metadata - narrowing my eyes on principle of YAGNI: ABB already handles metadata decently well, including look up. So long as the decrypted file contains SOME data we can use to ID the file when we call on ABB existing metadata lookup we shoudln't need to fuck with metadata at all.

Use local mediainfo cli to scope these two files that I just pulled via libation: /Users/jstar/Music/Libation/Books

I used libation to DL and Decrypt them, loaded them into ABB, and used ABB existing metadata handling and lookfup features to embed all the tags we need. So I think there's room to trim the fat from the design and implementation shape of these new features even further.  Good pointson the 3 typoes of audible files (aax, aaxc, DASH/wildvine), I think I'd want ABB to display the detect the type of file being acquired. As you can see in the images, ABB is already displaying the Codec and decoder when the incomign file is selected. Seems trivial to expose AAX/AAXC/DASH/WIDLFINE (what are those called BTW?).
```

---

## Assistant — Turn 3

*Ran `mediainfo` on both Libation-decrypted files in `/Users/jstar/Music/Libation/Books/`. Key tag findings (both files):*

```
Billy Fingers (2014 source content, encoded 2021-04-09):
  Title              : The Afterlife of Billy Fingers
  Album              : The Afterlife of Billy Fingers: ...
  Album/Performer    : Annie Kagan
  Performer          : Annie Kagan
  Composer           : Susan Ericksen; David Colacci
  Publisher          : Brilliance Audio
  Recorded date      : 2014
  asin / AUDIBLE_ASIN: B00KQ31BPO
  AUDIBLE_ACR        : CR!K2B1J3M4F12NK5NCEBP7R3XZF6EC
  AUDIBLE_DRM_TYPE   : Widevine
  AUDIBLE_LOCALE     : us
  Cover              : 2400x2400 JPEG embedded
  Audio              : AAC LC, 22.05 kHz, 62.8 kb/s
  Chapters           : 36 named, with timecodes

Star Trek: The Motion Picture (2019 source, encoded 2024-10-14):
  Title              : Star Trek: The Motion Picture
  Performer          : Gene Roddenberry
  Composer           : Robert Petkoff
  Publisher          : Simon & Schuster Audio
  asin / AUDIBLE_ASIN: 1797101692
  AUDIBLE_DRM_TYPE   : Widevine
  Cover              : 2400x2400 JPEG embedded
  Audio              : USAC (xHE-AAC), 44.1 kHz, 115 kb/s, mp4a-40-42
  Chapters           : 32 named, with timecodes
```

YAGNI wins on metadata. And the mediainfo actually flipped my phase plan — there's a much bigger finding in this output than the YAGNI confirmation.

### Confirming YAGNI on metadata

Both files have everything ABB's existing lookup pipeline needs to identify them and more. Notably present in `Billy Fingers`:

- `Title`, `Album`, `Performer`, `Composer` (narrators), `Publisher`, `Recorded date`
- `asin: B00KQ31BPO`, `AUDIBLE_ASIN`, `AUDIBLE_ACR` (Audible's content fingerprint)
- Embedded **2400×2400 JPEG cover** (`Image / Type: Cover / 2400x2400`)
- 36 named chapters with timecodes
- Full HTML description in both `Comment` and `Title, more info`

Same shape on the Star Trek file (32 named chapters, 2400×2400 cover, ASIN `1797101692`, full description). That's well past "enough to ID the file" — it's already richer than what ABB's online lookup typically retrieves.

**Metadata seam dropped from the design.** Acquisition produces only `MaterializedAsset` (verified local path + provenance). No `RemoteMetadataObservation`. ABB's existing `search_online_metadata` + draft UI handle it from there, same as picker/drop.

### Bigger finding: AAX legacy may be historical

Both of your files are tagged `AUDIBLE_DRM_TYPE: Widevine` — **including** the 2014 Billy Fingers title at the classic Audible 22.05 kHz/AAC-LC legacy quality tier. Audible kept the legacy audio quality but migrated the *DRM/transport* to Widevine. The Star Trek file is the modern USAC tier you'd expect for Widevine.

This is the same `org.libation` freeform tag Libation writes from the license-response `DrmType` (`Source/FileLiberator/DownloadDecryptBook.cs:259-262`), so it's an authoritative provenance marker, not a guess.

**What this means for the phase plan**: my "P1 AAX → P2 AAXC → P3 Widevine" sequencing was based on a wrong premise. If your account+device combination is getting Widevine even for re-issued legacy content, then a v1 that handles only AAX/AAXC ships nothing useful. **Widevine is the v1 requirement, not the deferred-policy phase.**

Revised position:

| Path | Role in v1 |
| --- | --- |
| **Widevine (DASH/CENC)** | **Must-have for v1.** This is what Audible actually serves your account. The "is ABB taking on Widevine-key-extraction territory?" conversation is the *v1* conversation, not a Phase-3 conversation. |
| **AAXC (voucher-keyed AES-CBC)** | Worth implementing because it's cheap once the API client exists (license response just gives you key+IV directly, decrypt math is trivial AES-CBC). May fire as fallback for some titles/marketplaces. |
| **AAX legacy (activation-bytes AES-CBC)** | Probably skip. Historical curiosity for your catalog state. Can be added later if a real user hits it. |

This is the opposite of "safe and gradual" — it's the *less* phased plan, because the legacy paths offered no real product value as standalone phases.

The architectural design shape doesn't change: provider trait, MaterializedAsset boundary, vault, staging, atomic commit. What changes is the **scope conversation moves up**: the Widevine session blob handling (Android-shaped device registration + base64 device blob obtained from a community source like Libation's `.cdmurls.json`-style hosting + LicenseRequest protobuf + key extraction) lands in v1, not in some future phase. We make that decision now.

### Naming the three formats

Direct answers for the UI label:

| Audible-internal label | Container / transport | Cipher | Key source | Codec usually | Suggested ABB display |
| --- | --- | --- | --- | --- | --- |
| **AAX** | MP4 with `adrm` atom (`.aax`) | AES-128-CBC | 4-byte "activation bytes" tied to Audible account | AAC-LC, 32–64 kb/s | "Audible AAX (legacy)" |
| **AAXC** | MP4 with `adrm` atom (`.aaxc`) | AES-128-CBC | 16-byte key + 16-byte IV per title, delivered in download voucher | AAC-LC or HE-AAC | "Audible AAXC" |
| **Widevine** (Audible's `DrmType.Widevine`) | MPEG-DASH (`.mpd`) → fragmented MP4 (fMP4/CMAF) segments | **AES-128-CTR** (MPEG-CENC `cenc` scheme — note the cipher *mode* differs from AAX/AAXC) | Content keys extracted from a Widevine license-response protobuf; Audible doesn't run Widevine playback, it just uses the response as a key-delivery transport | xHE-AAC (USAC, `mp4a-40-42`) for modern; AAC-LC for re-issued legacy | "Audible Widevine (DASH)" |

Couple of clarifications worth knowing:

- "DASH" and "Widevine" are orthogonal in general (DASH is delivery, Widevine is DRM), but in Audible's specific use they always pair, so labeling them together as "Audible Widevine (DASH)" is accurate and matches the `AUDIBLE_DRM_TYPE` tag your files already carry.
- Widevine's three security levels (L1/L2/L3) only matter for actual Widevine *playback*. Audible's approach extracts the AES keys from the license response and bypasses Widevine playback entirely, which is why the "Widevine CDM blob" Libation uses is a serialized device identity, not a real CDM library.
- The cipher *mode* difference is real and important for the implementation: AAX/AAXC are CBC; Widevine is CTR. Two different decrypt code paths. xHE-AAC is also USAC (Unified Speech and Audio Coding) — same thing, two names. The Star Trek file's "USAC / xHE-AAC" display is correct; Apple AudioToolbox is the only common decoder for it (FFmpeg's native AAC doesn't decode xHE-AAC), which is why ABB correctly routed it to Apple AAC in the screenshot.

### "Source acquisition format" display: trivial

The signal already exists in two complementary places:

1. **Libation-decrypted files**: `AUDIBLE_DRM_TYPE` freeform tag on the M4B (`Widevine` / `Adrm`). ABB's existing metadata read just needs to look for it.
2. **Future native acquisition**: comes from the license-response classification (per Libation's `DownloadOptions.cs:62-74` decision tree) and lands in the provenance manifest alongside the MaterializedAsset.
3. **Heuristic fallback** for files lacking both: codec = USAC strongly implies Widevine route; 22 kHz AAC-LC without `AUDIBLE_DRM_TYPE` suggests classic AAX.

Suggested label is "Source: Audible Widevine (DASH)" rather than "DRM: Widevine," since the *file* is no longer DRM-protected; it's an acquisition-provenance label, not a current-state label. Less confusing for users.

This is one new field in the file inspector, fed from existing tag-read paths. No new IPC surface.

### Trimmed scope going into the spec

Net effect of the last two corrections:

- **Out**: `RemoteMetadataObservation`, `analyzeMaterializedAssets` as a separate IPC command, the "phase plan by format," the metadata-intent seam discussion entirely.
- **In**: One material decision moved earlier — Widevine session/key-extraction path is v1 scope, with the legal/sourcing question for the device blob answered upfront rather than deferred.
- **Unchanged**: provider trait shape, MaterializedAsset = path + provenance, vault adapter, staging + atomic commit, `LocalImportBridge` reuse, forbidden paths, boundary assertions, GPL contamination rule for agents.
- **New small surface**: "Source acquisition format" provenance field, exposed in the file inspector. Backfills automatically for Libation-imported files via the `AUDIBLE_DRM_TYPE` tag.

The remaining open decision before I write the spec is just one:

**Q: Are you OK with v1 native acquisition including Widevine session blob handling** (Android-device registration + base64 device blob obtained from a community source like Libation's `.cdmurls.json`-style hosting + LicenseRequest protobuf + AES key extraction)?

If yes → I write the spec and we go.
If you want a fallback "Libation already produces M4Bs into a watched folder" import path *in addition to* native (not as the primary acquisition) → also easy, and that's a one-evening feature regardless because it's literally "watch `~/Music/Libation/Books` and offer to import new files into ABB."

Either way, want me to draft `docs/specs/remote-acquisition.md` now?

---

## User — Turn 4

```
Conocisely, here's the oldest file I can download via libation from my account: '/Users/jstar/Music/Libation/Books/The 7 Habits of Highly Effective People/The 7 Habits of Highly Effective People: Powerful Lessons in Personal Change.m4b' - what's this file using (aax?) and where in mediainfo did you see that?

Answers to qeustions:
Yes, include widevine handling, I mean, why not? Again, officer safety doesn't belong here. The goal is a YAGNI focused path to capture legacy and modern encoded files. When you say "v1" that's officer saftefy talk plus the fact you had to ask. Again, if libation is currently doing it, as should ABB without becoming a libation clone and lib manager.
```

---

## Assistant — Turn 4

*Ran `mediainfo` on the 7 Habits file, grepped for DRM/codec/bitrate signals. Output:*

```
Format                                   : MPEG-4
Format profile                           : Base Media
Codec ID                                 : isom (iso2/mp41/M4A /M4B )
Overall bit rate mode                    : Variable
Overall bit rate                         : 137 kb/s
Recorded date                            : 2004
Encoded date                             : 2026-04-28 13:12:07 UTC
AACR                                     : CR!FBDQ9AN5N91GKCVZD2NNXT17BXXR
AUDIBLE_DRM_TYPE                         : Widevine
Format                                   : USAC
Format/Info                              : Unified Speech and Audio Coding
Codec ID                                 : mp4a-40-42
Bit rate mode                            : Variable
Bit rate                                 : 135 kb/s
Sampling rate                            : 44.1 kHz
```

Confirmed Widevine — three for three. Even the oldest title in your library was re-encoded to USAC/Widevine. AAX legacy is functionally extinct for active Audible distribution.

**The file**: `AUDIBLE_DRM_TYPE: Widevine`, codec USAC (xHE-AAC) `mp4a-40-42`, 44.1 kHz, 135 kb/s. Recorded date 2004, encoded 2026-04-28 — Audible re-mastered the original 1989 content into modern xHE-AAC for this delivery.

**Where I saw it in mediainfo**: the line `AUDIBLE_DRM_TYPE : Widevine` in the *General* track, in the block of freeform tags alongside `AACR`, `AUDIBLE_ACR`, `AUDIBLE_ASIN`, `AUDIBLE_LOCALE`, `prID`. Those are Apple iTunes-list freeform atoms (`com.apple.iTunes` / `org.libation` namespaces) that Libation writes at decrypt time from the license-response `DrmType` field. They survive mediainfo, ffprobe, and any standard MP4 tag reader.

Three files, three `Widevine`. Writing the spec.

*Then created `docs/specs/remote-acquisition.md` (sibling file). Spec content not duplicated here — see the spec for the full text. Summary of what landed:*

Spec written: `docs/specs/remote-acquisition.md`. Decision-complete, agent-resumable, deletes-on-completion.

**What's locked in based on our alignment**:
- Native acquisition, no sidecar
- Widevine support is v1 scope, not deferred — directly because of the three-file evidence (1989/2014/2019 source dates, all Widevine)
- AAXC included opportunistically (small marginal cost once the API client exists)
- AAX legacy explicitly out until a real user title proves it needed (single-PR addition when that day comes)
- No metadata-enrichment seam — YAGNI confirmed by mediainfo: embedded tags + 2400×2400 cover + named chapters are richer than what ABB's online lookup typically retrieves
- `RemoteSourceRuntime` becomes ABB's **sixth** grey-box Public API
- `LocalImportBridge` extracted from `handlers.ts:processFilePaths` as a prerequisite, not a follow-up
- GPL contamination rule binds agents implementing `src-tauri/src/remote_source/providers/audible/`: protocol observation allowed, copying Libation/AAXClean implementation prohibited
- File inspector adds a "Source" row labeled as acquisition provenance (`Audible Widevine (DASH)` / `Audible AAXC` / `Audible AAX (legacy)` / `unknown`), not as current DRM state since the output file is unencrypted

**Two implementation-time decisions deliberately deferred to the agent that hits them** (captured in-spec with recommendations so they don't get re-litigated):
- D4 — where the Widevine device identity blob comes from (community-hosted index recommended)
- B2 — loopback HTTP listener vs embedded webview for the OAuth callback (loopback-first recommended)

**Plan of work** is six phases (A foundations → B auth → C library → D acquisition → E import/UI → F validation), each with concrete tasks an implementing agent can pick up directly.

Sanity check before implementation kicks off: anything in the spec read wrong to you, especially the **forbidden paths list** (constraints 1–7) and the **non-goals**? Those are the parts that determine what future agents will and won't refuse to do, and they're easier to amend now than later.

---

## User — Turn 5

```
Favor: Keep @docs/specs/remote-acquisition.md untouched. Add one more doc that captures our entire chat here (minus thinking traces) in an MD file that maeks it clear disinction between my responess and yours. Copy the chat exactly so other agents can see how we arrived where we are now.
```

*(This file is the response to Turn 5. Spec at `docs/specs/remote-acquisition.md` left untouched.)*
