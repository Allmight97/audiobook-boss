# Audiobook Boss System Map

Read this map only for repository onboarding, unclear ownership, or a change
that crosses frontend/backend or multiple product owners. For an ordinary
local change, start with root `AGENTS.md`, the nearest nested `AGENTS.md`, and
the owning code and tests.

ABB is a local, single-user Tauri application. One owner holds each product
truth, views dispatch semantic intent, typed interfaces cross seams, and Rust
owns durable artifact and operation truth.

## Product Spine

Audiobook Boss turns messy audiobook inputs plus user intent into organized,
tagged, compatible M4B outputs.

```text
Import -> Inspect -> Decide -> Preflight -> Process -> Verify
```

- **Import:** discover or materialize local audio, then validate and analyze it.
- **Inspect:** probe audio, chapters, metadata, cover art, and compatibility.
  Cover-art load vs write-prep lives in `docs/cover-art-processing.md`.
- **Decide:** stage metadata, naming, destination, encoder, and collision intent.
- **Preflight:** resolve paths, capabilities, collisions, and the execution plan.
- **Process:** accept work, run jobs, publish progress, and settle cancellation or failure.
- **Verify:** report backend terminal truth and leave trustworthy artifacts on disk.

## Control Loop

```text
User intent
  -> Solid view
  -> App Runtime owner                   session truth + semantic intent
  -> private Effect workflow             async coordination + typed failure
  -> tauriClient                         TS/Rust adaptation
  -> thin Tauri command                  ingress validation
  -> Rust domain/runtime owner           policy + lifecycle authority
  -> explicit plan                       resolved work before side effects
  -> direct preview | WorkRuntime        foreground probe | accepted operation
  -> Audio / Metadata / Output owners    owned side effects
  -> snapshots + artifact readback
  -> Solid view renders terminal truth
```

Remote Source materializes provider-owned titles into ABB-owned staged local
files, then hands them to the normal Input owner. It is not a hidden processing
path.

Preview and accepted work are deliberately different lanes. Direct preview
uses `process_audiobook_files` and foreground progress events; it has no backend
cancel command. Final processing and metadata batch save use WorkRuntime,
operation-scoped cancellation, and backend-authored operation snapshots. The
nearest Processing and WorkRuntime guidance owns the exact event rules.

## Owner Topology

| Owner | Path | Truth owned |
| --- | --- | --- |
| App Runtime | `src/app/runtime` | Composition, Solid context, owner lifetime, disposal, test harness. |
| Frontend owners | `src/app/<owner>` | Runtime-scoped draft/session state and semantic intents. Each `index.ts` is its live export truth. |
| Solid views | `src/ui/<owner>` | Markup, interaction wiring, screen-local state, and owner-local CSS; no parallel business store. |
| UI Foundation | `src/ui/foundation` | Shared Solid primitives, semantic tokens, document/WebView base, theme, and density. |
| Tauri runtime boundary | `src/lib/tauri` | Frontend command/event/plugin adaptation, payload normalization, and error presentation. |
| Command ingress | `src-tauri/src/commands` | Thin validation/adaptation into Rust owners. Command registration lives in `src-tauri/src/ipc_contract.rs`. |
| Processing | `src-tauri/src/processing` | Preflight/execution plans, runner coordination, lifecycle vocabulary, direct progress, and terminal classification. |
| WorkRuntime | `src-tauri/src/work_runtime` | Accepted operation identity, immutable inputs, snapshots, retention, and operation cancellation. |
| Audio Engine | `src-tauri/src/audio` | Import facts, inspection, toolchain selection, media execution, staging, cleanup, and integrity facts. |
| Metadata Outcome | `src-tauri/src/metadata` | Intent validation/normalization, effective metadata, write plans, and container-aware finalization. |
| Output Artifact | `src-tauri/src/output_artifact` | Requested/resolved paths, collision review, replacement, final commit, and success truth. |
| App Settings | `src-tauri/src/app_settings` + `src/app/appSettings` | Durable preferences plus frontend hydration, accepted-value coordination, and durability state. |
| Remote Source | `src-tauri/src/remote_source` + `src/app/remoteSource` | Provider capabilities/auth, acquisition, staged materialization, supplemental assets, and purge. |
| Core crates | `crates/abb-*-core` | Pure domain facts and classifiers packaged for an existing owner; not additional product owners. |

Callers cross an owner's Public API Strip—the allowed import/export surface
named by its nearest `AGENTS.md`. They do not reach into private state, helpers,
generated invokers, provider payloads, or filesystem mechanisms.

## State And Lifetime

| State | Lifetime and owner | Rule |
| --- | --- | --- |
| Screen interaction | Solid view instance | Keep disclosure, focus, and transient input local. |
| Presentation resource | View instance, or an owner-private resource when workflows share it | Dispose listeners, cancellation, caches, and late completions with that instance. |
| Session truth | One App Runtime owner | Read through its view/accessor and change through semantic intent. Never mirror it in another writable store. |
| Workflow transient state | Private Effect workflow | Publish typed outcomes through the owner; do not expose live Effect programs. |
| Capability truth | Owning Rust runtime | UI renders accepted facts; it does not reproduce backend rule tables. |
| Accepted operation | WorkRuntime until retention/purge | Stable identity, immutable accepted inputs, backend snapshots, operation-scoped cancellation. |
| Durable preference | Rust App Settings store | Runtime owner accepts behavior before persistence records it. |
| Artifact truth | Metadata, Audio, Output, and final disk readback | Success follows commit/finalization and any load-bearing verification. |
| Provider secret/session | Backend Remote Source + OS credential store | Never cross into frontend state, logs, processing payloads, or metadata. |

## Where Truth Lives

| Question | Read |
| --- | --- |
| What does the product do? | `README.md` and the Product Spine above. |
| Who owns this behavior? | Owner topology above, then its public module and nearest `AGENTS.md`. |
| What crosses TS and Rust? | `src-tauri/src/ipc_contract.rs`, generated bindings, and `src/lib/tauri/client.ts`. |
| Why was a durable choice made? | The relevant entry in `docs/DECISIONS.md`; do not load the ledger without a decision question. |
| What might be worked next? | A relevant open issue, verified against `main`, the owning interface, and tests. Issue state is evidence, not authority. |
| Which command proves it? | `scripts/AGENTS.md`, the nearest owner guidance, and live package scripts. |
| What happened in a run? | Typed results, Work Center/Status Panel, artifact readback, then run-scoped logs as supporting evidence. |

When sources disagree, determine which source owns the question. Code and
executed proof show observed behavior; guidance states protected invariants; a
decision records durable rationale; an issue proposes mutable work. Reconcile a
mismatch instead of blending the sources.

## Cross-Owner Invariants

- Every processing job has exactly one terminal outcome: `success`, `skipped`, `cancelled`, or `failed`.
- UI renders backend terminal truth; it does not invent final status.
- Metadata `set`, `clear`, and `noop` intent remains distinct across the runtime boundary.
- Input, output, and artifact paths remain validated at their owning ingress, plan, or commit seam.
- Accepted WorkRuntime submissions keep stable identity and immutable accepted inputs.
- External-provider partial failure remains typed and explicit at the owning command.
- Generated bindings are regenerated, never hand-edited.
- Session truth has one runtime owner; module globals and view stores do not become a second copy.
- Final success follows output commit/finalization and truthful cleanup semantics.
