# Audiobook Boss System Map

This file is the compact product/system shape for Audiobook Boss. It is not a
full architecture spec. Use it to orient work before opening the owning code,
then verify behavior in code and tests.

## Product Spine

Audiobook Boss turns messy audiobook inputs plus user intent into organized,
tagged, compatible M4B outputs.

The product workflow is:

```text
Import -> Inspect -> Decide -> Preflight -> Process -> Verify
```

- **Import**: bring files into the app through drag/drop or picker flows.
- **Inspect**: probe audio, chapters, metadata, cover art, duration, and compatibility.
- **Decide**: choose metadata, naming, destination, encoder, and skip/overwrite policy.
- **Preflight**: check paths, collisions, output plans, and tool availability before work starts.
- **Process**: run jobs, emit progress, handle cancellation, skip, failure, and success.
- **Verify**: report terminal truth and leave trustworthy output artifacts on disk.

## Layer Model

Use these layers to locate ownership before changing behavior:

| Layer | Owns |
| --- | --- |
| Product intent | What the user believes they asked the app to do. |
| UI state | Selected files, edits, visible status, and enabled actions. |
| Frontend workflow coordination | Effect workflow owners for multi-boundary async orchestration, typed workflow errors, fakeable services, and terminal UI outcomes. |
| IPC contract | Command, event, and payload shapes crossing TS <-> Rust. |
| Backend lifecycle | Operation identity, queue/progress events, cancellation, skipping, terminal summaries, and finalization. |
| Artifact truth | Final files, tags, paths, and terminal results on disk. |

## Boundary Rule

UI expresses intent and renders truth. Rust produces durable truth. The IPC
boundary prevents the two from drifting.

## Design Bias

ABB favors owned boundaries that make truthful decisions close to the domain
they govern.

Prefer:

- outcome requests over caller-selected strategy paths
- artifact and container truth over labels, suffixes, or optimistic UI state
- explicit compatibility policy over silent workaround behavior
- small public subsystem surfaces over scattered helper knowledge
- verification that demonstrates user-visible truth, not just internal path selection

This is a bias, not a command to collapse subsystems into fewer files. Private
strategy modules are healthy when each owns a coherent mechanism and the public
boundary owns the rule callers depend on.

Use this bias to reduce caller knowledge, stale compatibility lore, duplicated
routing rules, and false facades. Do not use it to create generic managers,
controllers, routers, or facades that merely rename complexity without improving
ownership or verification.

## Core Truth Boundaries

- UI code routes runtime commands/events through `src/lib/tauri/client.ts`.
- `src/app/runtime/` owns one app-lifetime Effect Atom registry, the Solid
  provider, exact-version reactivity/binding seams, and disposal.
- Multi-boundary frontend orchestration lives in named Effect workflow owners;
  Solid views dispatch intent and render state.
- `tauriClient` adapts generated bindings from `src/lib/generated/tauri.ts`.
- Rust commands are registered in `src-tauri/src/ipc_contract.rs` and implemented under `src-tauri/src/commands/`.
- Processing plans are built before execution and reviewed before jobs run.
- Backend lifecycle vocabulary and event emission live under `processing` as a
  sub-owner with a small public API; it is not a standalone Grey-Box Public API.
- Audio engine execution owns media inspection, decoder/toolchain selection,
  encode/mux/staging behavior, and media-integrity facts behind a small public
  API.
- `abb-media-core` packages backend-neutral media facts, error kinds, progress,
  and provenance vocabulary for owners that must classify media without leaking
  FFmpeg, AAXClean, provider payloads, or metadata-tool details.
- Runtime settings controls render backend-owned capability facts for selectable
  encoder and concurrency settings; UI labels stay frontend-owned, but
  accept/reject facts stay with Audio Engine and Job Registry.
- Remote source acquisition materializes provider-owned titles into local staged
  files before normal file-list import; provider auth, sessions, acquisition
  jobs, Supplemental Assets, and purge behavior stay behind `RemoteSourceRuntime`.
- Run orchestration owns dispatch and side effects; terminal outcome helpers own final status normalization.
- WorkRuntime (`src-tauri/src/work_runtime/`) owns Work Center operation truth:
  operation identity, immutable accepted submissions, operation snapshots, and
  operation-scoped cancellation. It wraps `processing::run` as the executor and
  derives operation terminal status from the canonical
  `abb_processing_core::classify_run_terminal` classifier, never a parallel rule.
- Metadata intent is compiled at the TS boundary and preserved through Rust writes and readback.
- Processing adapters produce media artifacts; final results report what actually happened.

## Grey-Box Public APIs

Use these as the current durable ownership map for architecture work:

In general architecture language, each entry is being shaped toward a **deep
module**: a small interface hiding substantial implementation complexity. In
ABB repo language, a **Grey-Box Module** is the stricter working form of that
idea: Public API Strip (the allowed import/export surface), Private Cluster,
nested ownership rules, narrow boundary checks, and contract tests.

| Public API | Owns |
| --- | --- |
| Tauri Runtime Boundary | Frontend runtime calls, payload normalization, generated-binding adaptation, and event listener setup. |
| Processing Plan | Preflight and execution planning before jobs run. |
| Output Artifact Plan / Commit | Requested/resolved artifact paths, collision review, parent directory creation, and final artifact commit truth. |
| Metadata Outcome Plan | Metadata intent validation/normalization, metadata intent projection, source hydration, naming-safe metadata, write plans, and cover-art passthrough policy. |
| WorkRuntime | Accepted background operation identity, immutable accepted inputs, operation snapshots, operation-scoped cancellation, and Work Center event truth. |
| Status Panel Runtime | Processing launch controls plus foreground preview progress/results rendered as truthful user-visible status. |
| Audio Engine Deep Module | Local audio import metadata/discovery, media inspection, decoder/toolchain selection, audio execution, encode/mux/staging internals, cleanup, and media execution facts. |
| App Settings | Durable preference schema, defaults, validation, JSON storage under Tauri app config, and settings IPC commands. |
| RemoteSourceRuntime | Provider registry/capabilities, backend-only account auth, secret vault access, library scan, acquisition jobs, staged materialized source files, Supplemental Assets, and remote-source cleanup/purge behavior. |

Each Public API has a nearest nested `AGENTS.md` that lists the allowed import/export surface, private cluster, edit rules, and breaking-change triggers.

### Frontend Owner Strips

| Strip | Entry | Owns |
| --- | --- | --- |
| App Runtime | `src/app/runtime` | One app-lifetime Atom registry, Solid provider, compatibility seams, test harness, and disposal. |
| Output Plan | `src/app/outputPlan` | Output directory, naming, path preview, derived estimate, and collision review. Solid views: `src/ui/outputPanel`, `src/ui/collisionDialog`. |
| Input Session | `src/app/inputSession` | File-list session, import analysis, selection, order, and inspector projection. Solid views: `src/ui/fileList` (`FileListView`), `src/ui/fileImport`. |
| File List | `src/ui/fileList` | `FileListView`, pointer reorder, and cover thumbnails. List truth stays in Input Session. |
| File Import | `src/ui/fileImport` | Picker, drop, opened-file drain, and Remote dialog mount. Composes `FileListView`. |
| Status Panel | `src/app/processing` | Preview submit, status runtime, and `statusViewAtom`. Solid view: `src/ui/statusPanel` (`StatusPanelView`). |
| Encoder Panel | `src/ui/encoderPanel` | Encoder settings UI and encoding request config reads. |
| App Settings | `src/app/appSettings` | Settings hydration, dialog state, and durable preference coordination. Persistence + dialog view: `src/ui/appSettings`. |
| Metadata Form | `src/ui/metadataForm` | `MetadataFormView` text fields. Form truth stays in Metadata Session. |
| Metadata Session | `src/app/metadataSession` | Per-file metadata cache, pending draft/intent staging (`stageMetadataIntentPatch`), cover, tags, and batch-save. |
| Metadata Lookup | `src/app/metadataLookup` | Lookup workflow, queue, and cover-preview scheduling. Solid dialog: `src/ui/metadataLookup` (`MetadataLookupView`). |
| Tag Preview | `src/ui/tagPreview` | `TagPreviewView`. TSOA and tag projection live in `src/app/metadataSession/tags.ts`. |
| Remote Source | `src/app/remoteSource` | Account/acquisition workflow, Input handoff, and session-asset retain/purge. Solid dialog: `src/ui/remoteSource`. |

Each strip's `index.ts` is exact export truth. Nearest `AGENTS.md` files record
only owner boundaries and recurring traps that are not cheap to infer from it.

Boundary-aligned Rust core crates under `crates/abb-*-core` are testing and
packaging surfaces for pure domain logic inside these owners. They are not new
Grey-Box Public APIs. `abb-media-core` is the first backend-neutral media
contract package; media execution still belongs to Audio Engine. `src-tauri`
remains the runtime shell for IPC, filesystem, keychain, FFmpeg/audio execution,
generated bindings, and cross-boundary verification.

Backend Lifecycle is a named sub-owner inside `processing`, not its own
Grey-Box Public API. It provides `OperationKind`, progress/queue event
vocabulary, job cancellation checks, and shared terminal-summary vocabulary.
WorkRuntime owns accepted background operation identity, snapshots, and
operation-scoped cancellation for Work Center operations.

## Core Invariants

- Every processing job has exactly one terminal outcome: `success`, `skipped`, `cancelled`, or `failed`.
- UI renders backend terminal truth; it does not invent final status.
- Metadata `set`, `clear`, and `preserve` intent stays distinct across the boundary.
- Path validation remains active for file inputs, output locations, and artifact writes.
- External provider partial failure stays explicit through typed diagnostics or terminal failure at the owning command.
- Generated bindings are not hand-edited.

## Task Frame

For meaningful work, identify:

- user/product outcome
- layer touched
- owned truth
- boundaries involved
- invariants protected
- done evidence

Use [docs/api-map.md](api-map.md) for the runtime command/event index and
[docs/ubiquitous-language.md](ubiquitous-language.md) for canonical terms.
