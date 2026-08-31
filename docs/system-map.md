# Audiobook Boss System Map

This is ABB's one orientation door: the compact product shape, ownership map,
state/control model, and context route. Use it to find the owning module and
the smallest truthful proof surface, then verify behavior in code and tests.
It is not a second implementation spec.

ABB is a local, single-user Tauri application. Its architecture is
owner-oriented: one module owns each product truth, views dispatch semantic
intent, typed interfaces cross seams, and Rust owns durable artifact truth.

The frontend direction is one disposable App Runtime with runtime-scoped
owners and view-instance presentation resources. `main` still has known
module-state exceptions in Processing, Encoding Configuration, Remote Source,
Output, Metadata, Metadata Lookup cover previews, and File List thumbnails;
issue [#471](https://github.com/Allmight97/audiobook-boss/issues/471) owns their
current convergence plan. Automatic settings persistence also still sits in a
UI strip and hides failures; [#421](https://github.com/Allmight97/audiobook-boss/issues/421)
owns moving that policy into App Settings. Treat these as existing gaps, not
patterns to copy.

## Product Spine

Audiobook Boss turns messy audiobook inputs plus user intent into organized,
tagged, compatible M4B outputs.

```text
Import -> Inspect -> Decide -> Preflight -> Process -> Verify
```

- **Import**: materialize or discover local audio, then validate and analyze it.
- **Inspect**: probe audio, chapters, metadata, cover art, duration, and compatibility.
- **Decide**: stage metadata, naming, destination, encoder, and collision intent.
- **Preflight**: resolve paths, capabilities, collisions, and the exact processing plan.
- **Process**: accept work, run jobs, publish progress, and settle cancellation or failure.
- **Verify**: report backend terminal truth and leave trustworthy artifacts on disk.

## System Control Loop

```text
User intent
  -> App Runtime owner state                 draft/session truth
  -> Effect workflow owner                  async orchestration + typed failure
  -> tauriClient                            TS/Rust adaptation seam
  -> Tauri command adapter                  ingress validation only
  -> Rust domain/runtime owner              policy + lifecycle authority
  -> explicit plan                          resolved work before side effects
  -> preview direct lane | WorkRuntime      foreground probe | accepted operation
  -> Audio / Metadata / Output owners       side effects behind owned interfaces
  -> operation snapshots + artifact readback
  -> Solid views render terminal truth
```

Remote Source joins this loop by materializing provider-owned titles into
ABB-owned staged local files. Those files then enter the normal Input Session;
remote acquisition never becomes a hidden processing path.

Every arrow is a seam. A caller crosses the owning interface; it does not reach
through to a private strategy, state store, generated invoker, provider payload,
or filesystem mechanism.

## Truth And Context Ladder

Each source owns a different question. Load only the row the task needs.

| Question | Owner | Durability and use |
| --- | --- | --- |
| What does the product do? | `README.md` + this Product Spine | Durable product shape. |
| Which module owns this truth? | This map, then the module's `index.ts` / Rust public module | Current navigation; verify the interface in code. |
| What must never drift here? | Root + nearest `AGENTS.md`, types, schema, and contract tests | Durable invariant and path-local operating rules. |
| What crosses TS and Rust? | `src-tauri/src/ipc_contract.rs`, generated bindings, `tauriClient`; `docs/api-map.md` is the index | Executable contract first; map for discovery only. |
| Why was a non-obvious choice made? | `docs/DECISIONS.md` | Current durable rationale and guardrail; git owns superseded history. |
| What work is active or next? | Open GitHub issue | Mutable plan/current state. Closed issues are history, not instructions. |
| Which command proves it? | `scripts/AGENTS.md`, nearest owner guidance, `package.json` | Current native command and expected signal. |
| What happened in a run? | Work Center / Status Panel, command results, `.logs/tauri-dev-summary.md`, run-scoped logs | Observed evidence; logs support truth but do not replace typed results or artifact readback. |
| What does an external library actually do? | Resolved manifest/lockfile source plus `.agents/skills/abb-library-research` | Version-specific evidence; do not cache upstream implementation snapshots in the repo. |

Do not merge conflicting sources into a compromise. Identify which owner
answers the question: code and executed proof show observed behavior; guidance
and decisions state the protected invariant; an open issue states intended
change. A mismatch is a finding to reconcile, not permission to pick the most
convenient source.

## Abstraction Tower

| Level | Owns | Must not own |
| --- | --- | --- |
| Product intent | The user outcome and compatible audiobook artifact they asked for. | Mechanism or adapter selection. |
| Owner intent | Semantic actions such as import, stage, review, submit, cancel, or persist. | Raw state setters and poke interfaces. |
| Session truth | Runtime-scoped Input, Metadata, Encoding Configuration, Output, Settings, Remote Source, Processing, and Work Operations state. | A second module-global or view-local copy. |
| Workflow coordination | Effect programs that coordinate async seams, resources, cancellation/lifetime handoff, and typed failure. | Public Effect programs or ordinary screen state. |
| Runtime Boundary | Command/event/payload adaptation across TypeScript and Rust. | Product policy or caller-side fallback. |
| Backend owners | Validation, planning, lifecycle, media execution, metadata outcomes, output commits, settings, and remote acquisition. | View state or provider details outside their owner. |
| Core libraries | Pure domain facts and classifiers packaged for an existing owner. | Runtime adapters, IPC, filesystem, keychain, or FFmpeg. |
| Adapters | Tauri, filesystem, FFmpeg, AAXClean, HTTP/provider, keychain, and platform mechanisms. | Product rules that callers must reconstruct. |
| Durable world | Preferences, final files/tags/paths, terminal reports, and external provider state. | Optimistic UI claims. |

The tower narrows upward: a user asks for an outcome; each lower module absorbs
mechanism and returns a smaller, more truthful interface. Complexity that leaks
upward must buy a real caller decision or move back behind its owner.

## State Model

| State class | Owner and lifetime | Control rule |
| --- | --- | --- |
| Screen-local interaction | Solid view instance; popover, disclosure, focus, transient input | Keep local unless another owner needs the truth. |
| Presentation resource | View instance or an owner-private resource when a workflow also consumes it; thumbnail/cover preview cache and scheduler | Scope cancellation, listeners, and late completion to that instance; never use a resettable module cache as accidental sharing. |
| Draft/session truth | One App Runtime owner instance | Read through a view/accessor; change through semantic owner intent. |
| Derived projection | Memo/view derived from owner truth | Recompute; do not mirror as independently writable state. |
| Workflow transient state | Named Effect workflow plus injected services | Keep the Effect program private; publish typed outcomes through the owner. |
| Capability truth | Audio Engine, Job Registry, App Settings, or Remote Source backend owner | UI renders backend facts; it does not reproduce acceptance tables. |
| Accepted operation truth | WorkRuntime until retention/purge | Immutable accepted input identity; backend-authored snapshots and operation-scoped cancellation. |
| Durable preference truth | Rust App Settings JSON store | Runtime owner accepts behavior first; persistence records the accepted preference. |
| Artifact truth | Metadata, Audio Engine, and Output Artifact owners plus final disk readback | Success follows commit/finalization and, where load-bearing, artifact verification. |
| Provider secret/session truth | Backend RemoteSourceRuntime and OS credential store | Never cross into frontend state, logs, processing payloads, or metadata. |
| Process instrumentation | Webview/bootstrap singleton such as the bounded frontend log bridge | May keep install/rate-limit counters only; never product, draft, operation, or artifact truth. |

Disposal is part of every session-state interface. Async generations, event
subscriptions, temporary resources, and late completions must not outlive or
repopulate their owner.

## Interface And Module Rules

ABB uses **Grey-Box Modules** for major owned subsystems: a small Public API
Strip, a hidden Private Cluster, local invariants, and proof through the same
interface callers use. In general architecture language, the goal is a deep
module: high leverage for callers and high locality for maintainers.

Prefer:

- outcome requests over caller-selected strategy paths;
- semantic intents over setters, binders, refresh calls, or state mirrors;
- artifact/container truth over labels, suffixes, or optimistic UI state;
- explicit compatibility or degradation policy over silent substitutes;
- one adapter seam only when behavior actually varies there;
- verification of user-visible truth over implementation-path assertions.

Do not create generic managers, controllers, routers, facades, or package
boundaries that only rename the same complexity. Apply the deletion test: if
removing a module merely removes a name, it is shallow; if its complexity
redistributes across callers, it is earning its interface.

## Core Truth Boundaries

- UI runtime commands/events route through `src/lib/tauri/client.ts`.
- `src/app/runtime/` composes the disposable Solid App Runtime and exposes it
  through context. Views consume owner interfaces from that context.
- `src/ui/foundation/` owns shared visual behavior: typed Solid primitives and
  public semantic CSS tokens. Native CSS is the only styling language.
- Named Effect workflow owners coordinate multi-boundary frontend work; Solid
  views dispatch intent and render state.
- `tauriClient` adapts generated bindings from `src/lib/generated/tauri.ts`.
- Rust commands are registered in `src-tauri/src/ipc_contract.rs` and remain
  thin adapters under `src-tauri/src/commands/`.
- Processing builds and reviews plans before execution.
- Backend Lifecycle owns operation vocabulary, progress/queue emission,
  cancellation checks, and terminal-summary normalization inside Processing.
- WorkRuntime owns accepted background operation identity, immutable accepted
  submissions, operation snapshots, retention, and operation-scoped cancellation.
- Audio Engine owns media inspection, decoder/toolchain selection,
  encode/mux/staging, cleanup, and media-integrity facts.
- Metadata Outcome owns validation/normalization, intent projection, naming-safe
  metadata, write plans, and container-aware finalization.
- Output Artifact owns requested/resolved paths, collision review, parent
  directories, replacement, final commit, and success wording.
- RemoteSourceRuntime owns provider registry/capabilities, auth/secrets,
  acquisition, staged materialization, Supplemental Assets, and purge.
- App Settings stores durable preferences; Audio Engine and Job Registry still
  own whether runtime values are accepted.
- Processing adapters produce media; terminal results report what actually
  happened rather than the route the caller hoped ran.

## Grey-Box Public APIs

| Public API | Owns |
| --- | --- |
| Tauri Runtime Boundary | Frontend runtime calls, payload normalization, generated-binding adaptation, and listener setup. |
| Processing Plan | Side-effect-free preflight and reviewed execution planning. |
| Output Artifact Plan / Commit | Requested/resolved paths, collision review, parent directories, replacement, and final commit truth. |
| Metadata Outcome Plan | Intent validation/normalization, effective/naming metadata, write plans, and cover policy. |
| WorkRuntime | Accepted operation identity, immutable inputs, snapshots, retention, cancellation, and Work Center truth. |
| Status Panel Runtime | Foreground preview controls, progress, results, and truthful terminal rendering. |
| Audio Engine Deep Module | Import facts, inspection, toolchain/adapter selection, execution, staging, cleanup, and media facts. |
| App Settings | Durable schema, defaults, validation, JSON storage, and settings commands. |
| RemoteSourceRuntime | Provider-neutral capabilities, backend auth/secrets, library/acquisition, staged files, Supplemental Assets, and purge. |

Backend owners have nearest `AGENTS.md` files for their public strips and
non-obvious traps. Rust core crates under `crates/abb-*-core` package pure
logic for these owners; they are not additional product owners.

## Frontend Owner Strips

For each existing owner, `src/app/<owner>/index.ts` is exact export truth.
`src/app/AGENTS.md` owns the common owner interface/lifetime rules; a nested
owner `AGENTS.md` adds only material local invariants. Tests and production
callers should cross the same owner interface. The one planned strip below is
marked explicitly and cannot be imported until #471 lands it.

| Strip | Entry | Owns |
| --- | --- | --- |
| App Runtime | `src/app/runtime` | Owner composition, Solid context, test harness, and disposal. |
| Input Session | `src/app/inputSession` | Import analysis, file session, selection, order, order lock, and inspector projection. |
| Metadata Session | `src/app/metadataSession` | Per-file metadata cache, form/draft intent, cover, tags, validation, staging, and batch save. |
| Metadata Lookup | `src/app/metadataLookup` | Lookup workflow, queue/apply, provider diagnostics, and cover-preview scheduling. |
| Encoding Configuration | `src/app/encodingConfig` (target owned by #471) | Runtime capabilities, current encoder/sample-rate/channel intent, request projection, defaults handoff, and estimate facts. |
| Output Plan | `src/app/outputPlan` | Directory, naming, backend path preview, estimate, and collision review. |
| Processing | `src/app/processing` | Preview submit, request composition, and Status Panel runtime. |
| Work Operations | `src/app/workOperations` | WorkRuntime read model, operation cancel/open, and purge tombstones. |
| Remote Source | `src/app/remoteSource` | Account/acquisition workflow, selection, Input handoff, and session assets. |
| App Settings | `src/app/appSettings` | Settings hydration, dialog state, startup/default coordination, and accepted runtime values. |
| UI Foundation | `src/ui/foundation` | Shared Solid primitives, semantic tokens, document/WebView base, theme, and density. |

Solid views live under `src/ui/<owner>` and own markup, interaction wiring, and
owner-local CSS. They do not own a parallel session store. Composition shells
may arrange public views but do not acquire business truth.

`src/app/encodingConfig` does not exist on `main` yet: encoder request truth,
capability loading, and listeners remain under `src/ui/encoderPanel`. The path
above is the explicit #471 target, not an importable current interface. The
same issue moves Metadata Lookup and Remote Source preview resources onto their
owners and makes File List's thumbnail resource view-scoped.

## Libraries And Adapters

- `abb-*-core` packages are pure libraries extracted at existing owner seams so
  domain logic can be reused and tested without the Tauri/runtime shell.
- `abb-media-core` supplies backend-neutral media facts, errors, progress, and
  provenance without moving media execution out of Audio Engine.
- `src/lib/effect/appEffect.ts` is ABB's only frontend Effect package ingress;
  owner workflows hide Effect behind Promise/synchronous owner interfaces.
- Solid is the renderer and reactive owner substrate, not a second business
  domain or workflow owner.
- Tauri, generated bindings, FFmpeg, AAXClean, provider HTTP, filesystem, and
  keychain code are adapters. Product policy stays on the owning side of each
  seam.
- Vendored `ffmpeg-sys-next` is build provenance with an explicit decision;
  general upstream source snapshots are not repository memory.

## Error And Diagnostic Semantics

| Condition | Contract |
| --- | --- |
| User-correctable invalid intent | Return structured validation/preflight data at the owner that can render and correct it. |
| Command/runtime failure | Rust returns `CommandResult<T>` / `AppErrorEnvelope`; `tauriClient` and `appError.ts` normalize category, cancellation, safe logging, and user message. |
| Expected provider partial failure | Return usable results plus typed diagnostics when the selected contract is still satisfied; otherwise fail at the owning command. |
| Cancellation | Preserve it as cancellation/terminal state, not a generic error or success. Background cancellation is operation-scoped. |
| Background completion | WorkRuntime publishes backend-authored snapshot status and terminal summary derived from the canonical Processing classifier. |
| Cleanup/finalization failure | Do not report success before cleanup-sensitive commit/finalization truth is known. |
| Unexpected developer/runtime failure | Sanitize and record enough local evidence; logs never become an alternate payload or terminal classifier. |

No caller reclassifies a typed failure by regex, invents fallback success,
suppresses provider degradation, or converts an empty/sentinel value into
metadata clear intent.

## Observability And Control Surfaces

| Surface | What it proves | What it does not prove |
| --- | --- | --- |
| Work Center | Accepted background operation identity, progress, children, cancellation, and terminal summary from WorkRuntime. | Foreground preview or final artifact contents by itself. |
| Status Panel | Direct foreground preview progress and backend terminal verdict. | Background operation truth or cancellability of the backend preview. |
| Command result / typed diagnostic | The owning command's accepted, rejected, degraded, or terminal response. | The final file if artifact readback is the load-bearing outcome. |
| Final artifact probe/readback | Files, container facts, metadata, chapters, duration, and path that actually exist. | UI presentation or external-player UX. |
| `.logs/tauri-dev-summary.md` | Latest captured dev session's semantic log verdict and lifecycle closure. | A substitute for typed command, snapshot, or artifact truth. |
| `.logs/runs/<run-id>/` | Bounded raw evidence for a named run. | Durable repository memory or a public contract. |
| Contract/owner tests | A plausible regression through the stable owning interface. | Visual judgment, subjective audio quality, or unexecuted external adapters. |
| Design lab / human visual pass | Foundation vocabulary and user-facing visual/interaction outcome. | Business workflow or Rust artifact behavior. |

## Agent Change Loop

1. **Frame the outcome.** Name the product-spine stage, user-visible terminal
   truth, and proof burden.
2. **Locate the owner.** Use this map, then open its public strip, nearest
   `AGENTS.md`, and the smallest representative test. Do not scan every layer.
3. **Separate state from plan.** Verify `main`; load an open issue only when it
   owns active work. Treat closed issues, branches, and chat as history until
   current evidence revives them.
4. **Trace only crossed seams.** Name state owner, semantic intent, workflow,
   IPC payload, backend owner, error/degradation path, and side effect only
   where the change crosses them.
5. **Change the owning module.** Move truth inward before extracting helpers;
   delete displaced mirrors, binders, aliases, and stale guidance.
6. **Prove at the lowest stable interface.** Escalate to contract, media,
   visual, or native proof only when the outcome depends on that seam.
7. **Accrete knowledge once.** Update the system map for ownership, API map for
   runtime command/event routing, glossary for behavior-changing terms,
   decision ledger for durable rationale, and the open issue for mutable plan
   state. Delete superseded copies.

## Core Invariants

- Every processing job has exactly one terminal outcome: `success`, `skipped`,
  `cancelled`, or `failed`.
- UI renders backend terminal truth; it does not invent final status.
- Metadata `set`, `clear`, and `preserve/noop` intent stays distinct across the boundary.
- Input, output, and artifact paths remain validated at their owning ingress/plan/commit seams.
- Accepted WorkRuntime submissions keep stable identity and immutable accepted inputs.
- External provider partial failure remains typed and explicit.
- Generated bindings are regenerated, never hand-edited.
- Session truth has one runtime owner; legacy module globals do not justify new copies.
- Final success follows output commit/finalization and truthful cleanup semantics.

Use [docs/api-map.md](api-map.md) only for command/event discovery,
[docs/ubiquitous-language.md](ubiquitous-language.md) for canonical terms, and
`scripts/AGENTS.md` for the owner-scoped command menu.
