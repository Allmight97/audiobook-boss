# Ubiquitous Language

## Repo And Runtime Boundaries
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Audiobook Boss** | A macOS desktop app for converting, tagging, and organizing audiobook files with durable metadata and processing workflows. | ABB app, converter only, script bundle |
| **Canon Surface** | A compact repo document that points to current truth and owning code without trying to duplicate the full implementation contract. | random docs, secondary truth, full spec dump |
| **Boundary** | An ownership seam where data or control crosses layers and must be normalized, validated, and kept contract-correct. | edge, glue, adapter noise |
| **Runtime Boundary** | The active TS↔Rust seam used by the app at runtime, centered on `tauriClient` rather than direct generated invokers. | generated seam, raw invoke layer |
| **Contract** | The explicit shape and meaning of commands, events, and payloads that must stay aligned across codegen, adapters, and consumers. | implicit behavior, loose shape |
| **Generated Bindings** | The committed generated TypeScript export of the Rust IPC contract used for drift detection and typed integration, not as the main handwritten runtime seam. | primary client, handwritten contract |
| **tauriClient** | The frontend-owned adapter that wraps generated commands/events and centralizes normalization, denormalization, and error handling. | raw invoke usage, direct generated calls |
| **IPC Contract** | The Rust-declared set of commands and events exported through `src-tauri/src/ipc_contract.rs` and consumed through generated bindings plus adapters. | ad hoc API, implicit bridge |
| **App Settings** | The backend-owned Grey-Box Module for durable user preference truth: schema, defaults, validation, private JSON storage, and settings IPC commands. Runtime owners still accept runtime-coupled changes before App Settings persists them. | localStorage settings, UI preference bag, settings plugin surface |
| **Runtime Settings Capabilities** | Backend-owned selectable settings facts exposed through the Tauri Runtime Boundary, currently covering encoder options and max-concurrent-job capabilities. The UI uses these facts to render controls and build requests without carrying independent validity tables. | frontend settings matrix, UI capability guess, settings fallback table |

## Processing And Metadata
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Job Registry** | The backend-owned active-job surface that tracks permits, queue state, and cancellation. | queue helper, incidental state |
| **Processing Flow** | The end-to-end audiobook processing path coordinated through `process_audiobook_files`, progress events, and processor stages. | encode call, single task |
| **Product Spine** | The user workflow from import through verification: Import, Inspect, Decide, Preflight, Process, Verify. | feature list, screen order |
| **Local Audio Import Boundary** | The frontend import workflow that all local audio ingress uses after a path exists: file picker, recursive folder picker, drag/drop, and OS Open With all route through backend import discovery, backend analysis, metadata draft staging, duplicate handling, and file-list append. Rust owns importable-audio truth; the frontend does not mirror supported extensions. | file picker helper, drop handler, frontend allowlist |
| **Product Intent** | What the user believes they asked Audiobook Boss to do with selected files, metadata, output rules, and processing settings. | UI state, backend guess |
| **UI State** | The frontend-held state for selected files, edits, visible status, and enabled actions before or after backend truth is returned. | product truth, backend state |
| **Backend Lifecycle** | The `processing` sub-owner/public strip for operation identity, queue/progress event vocabulary, cancellation checks, and terminal-summary truth used by long-running backend work. Not a standalone Grey-Box Public API. | processing helper, job loop, status UI owner |
| **Operation Kind** | The backend-declared operation identity carried on lifecycle events, currently `processingMerge`, `processingBatch`, or `metadataSave`. | caller mode guess, UI-only work kind |
| **Operation Result Summary** | Shared terminal counts for long-running backend operations: total, succeeded, skipped, cancelled, and failed. | processing-only summary, UI completion guess |
| **Artifact Truth** | The final files, tags, paths, and terminal results that exist after processing or metadata operations complete. | expected output, UI summary |
| **Terminal Outcome** | The final per-job status after processing ambiguity resolves: `success`, `skipped`, `cancelled`, or `failed`. | progress stage, current status |
| **Terminal Truth** | The backend-owned final report of what happened to a job or batch, used by the UI for user-visible completion state. | optimistic status, frontend truth |
| **Metadata Intent Patch** | An explicit patch-op payload that preserves whether a metadata field is being set, cleared, or left alone. | raw metadata object, sentinel mutation |
| **Patch Op** | A single explicit metadata action such as `set`, `clear`, or `noop` used to preserve user intent across the boundary. | magic empty value, implicit clear |
| **Metadata Outcome Plan** | The metadata boundary's backend plan that turns source metadata plus intent into effective metadata, naming metadata, write instructions, and cover-art passthrough policy. | metadata helper chain, raw metadata object |
| **Processing Preflight Plan** | A backend-generated preview of how a processing request will execute before the actual long-running job begins. | dry guess, UI-only estimate |
| **Output Path Preview** | The backend-owned computation of the intended output path before collision suffixing or final processing writes. | frontend filename guess, audio preview |
| **Fallback** | A deliberately registered temporary compatibility or integrity path with a concrete trigger, observable signal, and sunset condition. | convenience workaround, silent compatibility shim |
| **Fallback Register** | The canonical list of still-active fallbacks that repo checks enforce and sunset-date. | misc exceptions, hidden legacy notes |

## Verification And Planning
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Proof Route** | A named route under `scripts/proof.sh` that chooses the cheapest meaningful proof for the touched owner surface. | random command, local incantation |
| **Standard Gate** | The main non-doc review proof route run via `scripts/proof.sh standard` to validate code, contracts, and repo health. | test pass, quick smoke check |
| **UI Workflow Smoke Test** | A deterministic app-level test that exercises one high-value user path with mocked Tauri boundaries and asserts visible state transitions. | screenshot-only proof, vague scenario check |
| **Active Spec** | A temporary `docs/specs/<task>.md` work packet for substantial planning, roadmap, architecture, or implementation work. It is self-contained while active and must be deleted or distilled into canon when done. | permanent feature doc, transcript, generated report |
| **Source Of Truth** | The owning code or canon surface that should settle a question before implementation claims are made. | memory, assumption, third-party critique |
| **Minimal Churn** | Fewer reactive correction loops and less avoidable rework, not automatically the smallest diff. | smallest patch, least code motion |

## System Ownership Terms
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Boundary Ownership** | The practice of giving one layer clear responsibility for validation, normalization, and shape translation so logic does not scatter. | shared everywhere, whoever touches it |
| **Contract Truth** | The discipline of making code, generated artifacts, docs, and tests tell the same story about a cross-layer interface. | close enough, implied compatibility |
| **Operational Truthfulness** | The requirement that progress, queue state, failures, and user-visible status reflect what the system is actually doing. | optimistic UI, silent failure |
| **Durable Workflow Surface** | A repo artifact that lets a future agent or engineer resume work without depending on chat memory or guesswork. | transcript archaeology, informal notes |

## Grey-Box Module Vocabulary
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Grey-Box Module** | ABB's repo-governed form of a **Deep Module**: a small, deliberately published **Public API Strip** backed by a hidden **Private Cluster** of implementation files, boundary assertions, and contract tests. Use this term inside ABB when the ownership rules matter; use **Deep Module** in general engineering discussion. | shallow module, façade-only wrapper, generic "module", deep/grey module |
| **Public API Strip** | The deliberately small set of public symbols (functions, types, events, commands) a grey-box module allows callers to import. Symbols outside the strip are not public even when the language would allow them to be. | exports list, "everything pub", surface area |
| **Private Cluster** | The set of files inside a grey-box module that implement its Public API Strip. Rename-safe, split-safe, AI-editable, and not importable from outside the module. | helper files, internal utilities (unscoped) |
| **Module Owner** | The single grey-box module a product decision or invariant belongs to. If two modules both feel partial responsibility, the rule has no owner. | shared responsibility, "wherever it ends up" |
| **Seven Public APIs** | The current ABB grey-box public-API set: Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, Status Panel Runtime, Audio Engine Deep Module, and App Settings. | "the modules" (ambiguous), deep modules (too broad) |
| **Audio Engine Deep Module** | The Grey-Box Public API owner for media inspection, decoder/toolchain selection, audio execution, encode/mux/staging internals, cleanup, and media execution facts. | processor helper, ffmpeg wrapper, generic media manager |
| **Reach-Through** | An import that crosses a module boundary into another module's Private Cluster. Always a smell; always names a bug, an unowned rule, or an unintentional contract. | shortcut, "just this once" |
| **Ownership Smear** | A product rule whose implementation is split across two or more modules where each holds a partial answer and no single source of truth exists. | shared concern, "it depends" |
| **Contract Test** | A test that pins the externally visible behavior of a grey-box module's Public API Strip. Internal cluster changes must keep contract tests green. | unit test, helper existence test |
| **Boundary Assertion** | A repo-level script check (the `scripts/check-no-bridge-imports.sh` family) that fails CI if a Reach-Through is reintroduced. | lint suggestion, code-review note |
| **Cluster Audit** | A non-mutating gut check of a Private Cluster's code shape: file size, function size, decision-per-function clarity, internal naming, and tests-close-to-behavior. Used to plan future internal refactors without changing the Public API Strip. | refactor sweep, "clean code pass" |
| **Deep Module** | The published Ousterhout term (*A Philosophy of Software Design*) for a module with a small interface and substantial hidden implementation. ABB **Grey-Box Modules** are deep-module-shaped ownership units, but the terms are not interchangeable: "deep module" names the design quality; "Grey-Box Module" names ABB's documented contract and edit model. | shallow façade, micro-module, grey module |
| **Information Hiding** | The published Parnas 1972 rule that callers must not depend on a module's implementation details. The reason the Private Cluster exists at all. | encapsulation (vague), abstraction (vague) |

## Relationships

- The **IPC Contract** is exported from Rust, materialized as **Generated Bindings**, and consumed through the **tauriClient** **Runtime Boundary**.
- A **Boundary** owns normalization and validation so **Contract Truth** does not leak into scattered UI callsites.
- The **Product Spine** moves from **Product Intent** through **UI State**, **IPC Contract**, **Backend Lifecycle**, and **Artifact Truth**.
- **App Settings** stores accepted durable preferences; runtime owners such as
  **Job Registry**, audio/toolchain validation, and output planning still accept
  or reject live runtime changes before those values become preference truth.
- **Runtime Settings Capabilities** expose selectable encoder and concurrency
  facts from those runtime owners; they are not durable preference storage.
- The **Backend Lifecycle** strip under `processing` names operation identity,
  queue/progress events, cancellation checks, and shared terminal summaries for
  processing and metadata save.
- The **Job Registry** is the authority for active **Processing Flow** jobs,
  queue state, permits, and cancellation.
- A **Terminal Outcome** contributes to **Terminal Truth** only after backend processing resolves the job's final state.
- A **Metadata Intent Patch** is compiled at the **Runtime Boundary** and preserved across the **IPC Contract** so clear intent is never inferred from sentinel values.
- A **Metadata Outcome Plan** is produced by the metadata boundary so processing and output callers consume effective metadata, naming metadata, write facts, and cover-art policy instead of rebuilding metadata sequencing.
- A **Fallback** must appear in the **Fallback Register** and stay observable until it is removed or renewed.
- The **Standard Gate** and focused **UI Workflow Smoke Test** coverage are proof surfaces for keeping **Contract Truth** and **Operational Truthfulness** honest.
- An **Active Spec** is a temporary **Durable Workflow Surface** for substantial work produced through **decision-alignment**; it complements, but does not replace, canon repo docs.
- A **Deep Module** is the general architecture idea; a **Grey-Box Module** is ABB's stricter repo pattern for applying it.
- A **Grey-Box Module** publishes a **Public API Strip** and hides a **Private Cluster** behind it; only one **Module Owner** holds any given product rule.
- A **Reach-Through** is the diagnostic for an **Ownership Smear**; a **Boundary Assertion** is the script-enforced cure.
- Each **Public API Strip** in the **Seven Public APIs** set is locked by **Contract Tests**; internal cluster changes are safe when contract tests stay green. **Backend Lifecycle** is instead a sub-owner/public strip inside `processing`.
- A **Cluster Audit** inspects shape inside a **Private Cluster** without changing the **Public API Strip**; it informs future internal refactor decisions and does not, by itself, change behavior.

## Example Dialogue

> **Dev:** "Should the UI call the generated command directly when saving metadata?"
> **Domain expert:** "No. Route it through the **tauriClient** so the **Metadata Intent Patch** stays explicit at the **Runtime Boundary** and **Contract Truth** remains centralized."

## Flagged Ambiguities

- "API", "command surface", and "runtime boundary" can blur together. Prefer **IPC Contract** for the Rust-exported command/event set and **Runtime Boundary** for the handwritten TS adapter seam.
- "generated bindings" can sound like the primary client. Prefer **Generated Bindings** for drift detection and typed integration, and **tauriClient** for the real runtime adapter.
- "preview" can mean different things. Prefer **Output Path Preview** for path derivation, **Processing Preflight Plan** for pre-run backend review, **audio preview** for a short media render, and **preview artifact** for the file created by that render.
- "fallback" can drift into "temporary workaround." Prefer **Fallback** only for registered, observable, sunset-bound behavior; otherwise call it a compatibility path, recovery default, bug, or design decision.
- "metadata" and "metadata intent" are not interchangeable. Prefer **Metadata Intent Patch** when the important question is user intent to set, clear, or preserve a field; use **Metadata Outcome Plan**, metadata draft, write plan, lookup result, or tag projection when those narrower concepts are meant.
- "proof route", "gate", "check", "test", and "smoke test" are different confidence shapes. Prefer **Proof Route** for named `scripts/proof.sh` paths, **Standard Gate** for `scripts/proof.sh standard`, **Boundary Assertion** for repo scripts that block broken imports or policy drift, **Contract Test** for public API behavior, and **UI Workflow Smoke Test** for deterministic user-flow proof.
- Product names and implementation names should not blur together. For example, **Book Binder** is user-facing product language; **Merge** is the processing job type.
- "minimal churn" can be mistaken for "smallest diff." Prefer **Minimal Churn** as fewer correction loops and less rework, even when the better fix is somewhat broader.
- "status", "progress", and "terminal outcome" are not interchangeable. Prefer **Terminal Outcome** only for final per-job status and **Terminal Truth** for the backend-owned final report.
- "backend lifecycle" and "Status Panel Runtime" are not interchangeable. The
  lifecycle strip emits operation facts; Status Panel consumes them as a read
  model.
- "deep module" and "grey-box module" are related but distinct. Prefer **Deep Module** when talking to other engineers about the general design principle; prefer **Grey-Box Module** when referring to ABB's Public-API + Private-Cluster + boundary-assertion ownership unit.
- "module" alone is ambiguous in ABB. Prefer **Grey-Box Module** for the owned Public-API + Private-Cluster unit, and **Private Cluster File** for any implementation file inside one.
- "API" can mean three different things in ABB. Prefer **IPC Contract** for the Rust-declared command/event set, **Runtime Boundary** for the TS adapter (`tauriClient`), and **Public API Strip** for the externally allowed import set of any **Grey-Box Module**.
- "contract" can be ambiguous. Prefer **IPC Contract** for the cross-language command/event set and **Contract Test** for the behavior-locked test of any **Grey-Box Module's** Public API Strip.
- "refactor" can mean either internal cluster reshape or boundary change. Prefer "**Cluster Audit** + internal refactor" for changes inside a Private Cluster (no Public API Strip impact) and "**Public API Strip** change" for anything callers will notice.
