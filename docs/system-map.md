# Audiobook Boss System Map

Read this map only for repository onboarding, unclear ownership, or a change
that crosses frontend/backend or multiple product owners. For an ordinary
local change, start with root `AGENTS.md`, the nearest nested `AGENTS.md`, and
the owning code and tests.

ABB is a local, single-user desktop application. A Rust engine
(`crates/abb-engine`) owns the product's rules and the working session; the
Tauri app is one host for it. One owner holds each product truth, views
dispatch semantic intent, and typed interfaces cross seams. A native UI
would replace only the host tier.

## Product Spine

Audiobook Boss turns messy audiobook inputs plus user intent into organized,
tagged, chapterized audiobook files. AAC/M4B is the default; title audio plans
can choose MP3 pass-through or Opus in M4A/MKA.

```text
Import -> Inspect -> Decide -> Preflight -> Process -> Verify
```

- **Import:** discover or materialize local audio, then validate and analyze it.
- **Inspect:** probe audio, chapters, metadata, cover art, and compatibility.
  Cover-art load vs write-prep lives in `docs/cover-art-processing.md`.
- **Decide:** stage metadata, naming, destination, per-book audio handling, encoder, and collision intent.
- **Preflight:** resolve paths, capabilities, collisions, and the execution plan.
- **Process:** accept work, run jobs, publish progress, and settle cancellation or failure.
- **Verify:** report backend terminal truth and leave trustworthy artifacts on disk.

## Control Loop

Working session and settings (titles, selection, metadata edits, lookup,
Save, audio choices, output naming and estimates, submission, preferences):

```text
User intent
  -> Solid view
  -> src/app adapter owner               wording + local echo of typed text
  -> engineLink                          numbered intent
  -> tauriClient -> host command         run in the order sent (intent order)
  -> Engine session / settings runtime   rule, transition, file/network work
  -> reply + session/settings events     snapshot parts with revisions
  -> engineLink keeps newest parts -> Solid view renders
```

A submission continues inside the engine:

```text
Submit / Preview intent
  -> Engine session                      build the export from the session, hold its sources
  -> preflight plan                      paths, collisions, signature; ReviewRequired waits for a choice
  -> preview run | WorkRuntime           foreground probe | accepted operation
  -> Audio / Metadata / Output owners    owned side effects
  -> snapshots + artifact readback
  -> Solid view renders terminal truth
```

Remote Source materializes provider-owned titles into ABB-owned staged local
files, then the engine imports them into the working session like any other
files. It is not a hidden processing path.

Preview and accepted work are deliberately different lanes. A preview runs
in the session with foreground progress events; it has no backend cancel
command. Final processing and metadata batch save use WorkRuntime,
operation and title cancellation, and backend-authored operation snapshots. The
nearest Processing and WorkRuntime guidance owns the exact event rules.

## Owner Topology

| Owner | Path | Truth owned |
| --- | --- | --- |
| App Runtime | `src/app/runtime` | Composition, Solid context, owner lifetime, and disposal. |
| Engine link | `src/app/engineLink` | The frontend's one connection to the engine: newest snapshot parts and numbered intents. |
| Frontend owners | `src/app/<owner>` | Adapters over engine state (input, metadata, lookup, settings, encoding, output plan), or workflow still owned in TS (preview status, work operations, remote source). Each `index.ts` is its live export truth. |
| Solid views | `src/ui/<owner>` | Markup, interaction wiring, screen-local state, and owner-local CSS; no parallel business store. |
| UI Foundation | `src/ui/foundation` | Shared Solid primitives, semantic tokens, document/WebView base, and theme. |
| Tauri runtime boundary | `src/lib/tauri` | Frontend command/event/plugin adaptation, payload normalization, and error presentation. |
| Tauri host | `src-tauri` | Commands, intent ordering, event forwarding, window, quit handling (engine shutdown). Command registration lives in `src-tauri/src/ipc_contract.rs`. |
| Working session | `crates/abb-engine/src/session` | Titles and sources, selection, metadata edits and known tags, lookup, cover, Save targets, audio choices and title plans, output naming, path preview, size estimates, submission and collision review, exported-title links and restart offers, and imported downloads. |
| Processing | `crates/abb-engine/src/processing` | Preflight/execution plans, runner coordination, lifecycle vocabulary, direct progress, and terminal classification. |
| WorkRuntime | `crates/abb-engine/src/work_runtime` | Accepted operation identity, each title's output record, snapshots, retention, and operation cancellation. |
| Active-work power | `crates/abb-engine/src/power.rs` | One macOS idle-sleep hold across active work scopes, with immediate preference changes and release after the last scope ends. |
| Audio Engine | `crates/abb-engine/src/audio` | Import facts, inspection, encoder selection, media execution, staging, cleanup, and integrity facts. |
| Metadata Outcome | `crates/abb-engine/src/metadata` | Intent validation/normalization, effective metadata, write plans, and container-aware finalization. |
| Output Artifact | `crates/abb-engine/src/output_artifact` | Requested/resolved paths, collision review, replacement, final commit, and success truth. |
| App Settings | `crates/abb-engine/src/app_settings` + `src/app/appSettings` | Settings in effect, validation, storage, and durability in the engine; dialog state and wording in TS. |
| Remote Source | `crates/abb-engine/src/remote_source` + `src/app/remoteSource` | Provider capabilities/auth, acquisition, and staged materialization in the engine (the session imports the files and decides when downloads go); dialog state and wording in TS. |
| Core crates | `crates/abb-*-core` | Pure domain facts and classifiers packaged for an engine owner; not additional product owners. Tier rules: `crates/AGENTS.md`. |

Callers cross an owner's Public API Strip—the allowed import/export surface
named by its nearest `AGENTS.md`. They do not reach into private state, helpers,
generated invokers, provider payloads, or filesystem mechanisms.

## State And Lifetime

| State | Lifetime and owner | Rule |
| --- | --- | --- |
| Screen interaction | Solid view instance | Keep disclosure, focus, and transient input local. |
| Presentation resource | View instance, or an owner-private resource when workflows share it | Dispose listeners, cancellation, caches, and late completions with that instance. |
| Working session | Engine `Session`, one per engine | Hosts read snapshot parts and send intents. The frontend keeps only the newest parts and display-only echo of typed text. |
| Frontend workflow truth | One App Runtime owner | Read through its view/accessor and change through semantic intent. Never mirror it in another writable store. |
| Workflow transient state | Private workflow owner | Plain async; publish outcomes through the owner. |
| Capability truth | Owning Rust runtime | UI renders accepted facts; it does not reproduce backend rule tables. |
| Accepted operation | WorkRuntime until retention/purge | Stable identity, accepted inputs that change only through a title's output record (tags and cover), backend snapshots, operation and title cancellation. |
| Settings in effect | Engine settings runtime + JSON store | Runtime owner accepts behavior before it is recorded; a failed write keeps the setting in effect and retryable. |
| Waiting metadata write | Engine session until the export reading the file finishes | Survives removal of its title; shutdown cancels the export and writes it. |
| Artifact truth | Metadata, Audio, Output, and final disk readback | Success follows commit/finalization and any load-bearing verification. |
| Provider secret/session | Backend Remote Source + OS credential store | Never cross into frontend state, logs, processing payloads, or metadata. |

## Where Truth Lives

| Question | Read |
| --- | --- |
| What does the product do? | `README.md` and the Product Spine above. |
| Who owns this behavior? | Owner topology above, then its public module and nearest `AGENTS.md`. |
| What crosses TS and Rust? | `src-tauri/src/ipc_contract.rs`, generated bindings, and `src/lib/tauri/client.ts`. |
| Why was a durable choice made? | The rationale beside the rule: the owning `AGENTS.md` or the enforcing code's comment; history is in PR bodies and git. |
| What might be worked next? | A relevant open issue, verified against `main`, the owning interface, and tests. Issue state is evidence, not authority. |
| Which command proves it? | `scripts/AGENTS.md`, the nearest owner guidance, and live package scripts. |
| What happened in a run? | Typed results, Work Center/Status Panel, artifact readback, then run-scoped logs as supporting evidence. |

When sources disagree, determine which source owns the question. Code and
executed proof show observed behavior; guidance states protected invariants and
the rationale beside them; an issue proposes mutable work. Reconcile a
mismatch instead of blending the sources.

## Cross-Owner Invariants

- Every processing job has exactly one terminal outcome: `success`, `skipped`, `cancelled`, or `failed`.
- UI renders backend terminal truth; it does not invent final status.
- Metadata `set`, `clear`, and absent (keep source) intent stay distinct from the session's field edits through Save and processing.
- Input, output, and artifact paths remain validated at their owning ingress, plan, or commit seam.
- Accepted WorkRuntime submissions keep stable identity; after acceptance only a title's tags and cover change, through its `TitleOutput` (`crates/abb-engine/src/session/AGENTS.md`, Exported titles).
- External-provider partial failure remains typed and explicit at the owning engine module.
- Generated bindings are regenerated, never hand-edited.
- Session truth has one owner (the engine for the working session); module globals and view stores do not become a second copy.
- Save never writes a file while an accepted export reads it: a local source waits, a temporary download is not written (`crates/abb-engine/src/session/AGENTS.md`).
- Final success follows output commit/finalization and truthful cleanup semantics.
