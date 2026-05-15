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

## Processing And Metadata
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Job Registry** | The backend-owned concurrency lifecycle surface that tracks processing jobs, queue state, and cancellation. | queue helper, incidental state |
| **Processing Flow** | The end-to-end audiobook processing path coordinated through `process_audiobook_files`, progress events, and processor stages. | encode call, single task |
| **Product Spine** | The user workflow from import through verification: Import, Inspect, Decide, Preflight, Process, Verify. | feature list, screen order |
| **Product Intent** | What the user believes they asked Audiobook Boss to do with selected files, metadata, output rules, and processing settings. | UI state, backend guess |
| **UI State** | The frontend-held state for selected files, edits, visible status, and enabled actions before or after backend truth is returned. | product truth, backend state |
| **Backend Lifecycle** | The Rust-owned sequence that plans, queues, runs, cancels, skips, fails, succeeds, and finalizes processing work. | processing helper, job loop |
| **Artifact Truth** | The final files, tags, paths, and terminal results that exist after processing or metadata operations complete. | expected output, UI summary |
| **Terminal Outcome** | The final per-job status after processing ambiguity resolves: `success`, `skipped`, `cancelled`, or `failed`. | progress stage, current status |
| **Terminal Truth** | The backend-owned final report of what happened to a job or batch, used by the UI for user-visible completion state. | optimistic status, frontend truth |
| **Metadata Intent Patch** | An explicit patch-op payload that preserves whether a metadata field is being set, cleared, or left alone. | raw metadata object, sentinel mutation |
| **Patch Op** | A single explicit metadata action such as `set`, `clear`, or `noop` used to preserve user intent across the boundary. | magic empty value, implicit clear |
| **Processing Preflight Plan** | A backend-generated preview of how a processing request will execute before the actual long-running job begins. | dry guess, UI-only estimate |
| **Output Path Preview** | The backend-owned computation of the intended output path before collision suffixing or final processing writes. | frontend filename guess, audio preview |
| **Fallback** | A deliberately registered temporary compatibility or integrity path with a concrete trigger, observable signal, and sunset condition. | convenience workaround, silent compatibility shim |
| **Fallback Register** | The canonical list of still-active fallbacks that repo checks enforce and sunset-date. | misc exceptions, hidden legacy notes |

## Verification And Planning
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Standard Gate** | The main non-doc quality gate run via `scripts/checks.sh standard` to validate code, contracts, and repo health. | test pass, quick smoke check |
| **UI Workflow Smoke Test** | A deterministic app-level test that exercises one high-value user path with mocked Tauri boundaries and asserts visible state transitions. | screenshot-only proof, vague scenario check |
| **Task Spec** | The one active `docs/specs/<task>.md` working document produced through decision alignment for substantial multi-session or multi-lane execution. | scratchpad, session log |
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
| **Grey-Box Module** | An ABB ownership unit that pairs a small, deliberately published **Public API Strip** with a **Private Cluster** of implementation files. Formally: a deep module (Ousterhout) with strict information hiding (Parnas). The private cluster is AI-editable; the public API is the contract that locks behavior. | shallow module, façade-only wrapper, generic "module" |
| **Public API Strip** | The deliberately small set of public symbols (functions, types, events, commands) a grey-box module allows callers to import. Symbols outside the strip are not public even when the language would allow them to be. | exports list, "everything pub", surface area |
| **Private Cluster** | The set of files inside a grey-box module that implement its Public API Strip. Rename-safe, split-safe, AI-editable, and not importable from outside the module. | helper files, internal utilities (unscoped) |
| **Module Owner** | The single grey-box module a product decision or invariant belongs to. If two modules both feel partial responsibility, the rule has no owner. | shared responsibility, "wherever it ends up" |
| **Five Public APIs** | The current ABB grey-box public-API set: Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Intent Plan, Status Panel Runtime. | "the modules" (ambiguous) |
| **Reach-Through** | An import that crosses a module boundary into another module's Private Cluster. Always a smell; always names a bug, an unowned rule, or an unintentional contract. | shortcut, "just this once" |
| **Ownership Smear** | A product rule whose implementation is split across two or more modules where each holds a partial answer and no single source of truth exists. | shared concern, "it depends" |
| **Contract Test** | A test that pins the externally visible behavior of a grey-box module's Public API Strip. Internal cluster changes must keep contract tests green. | unit test, helper existence test |
| **Boundary Assertion** | A repo-level script check (the `scripts/check-no-bridge-imports.sh` family) that fails CI if a Reach-Through is reintroduced. | lint suggestion, code-review note |
| **Cluster Audit** | A non-mutating gut check of a Private Cluster's code shape: file size, function size, decision-per-function clarity, internal naming, and tests-close-to-behavior. Used to plan future internal refactors without changing the Public API Strip. | refactor sweep, "clean code pass" |
| **Deep Module** | The published Ousterhout term (A Philosophy of Software Design) for a module with a small interface and substantial hidden implementation. ABB calls these grey-box modules locally. | shallow façade, micro-module |
| **Information Hiding** | The published Parnas 1972 rule that callers must not depend on a module's implementation details. The reason the Private Cluster exists at all. | encapsulation (vague), abstraction (vague) |

## Relationships

- The **IPC Contract** is exported from Rust, materialized as **Generated Bindings**, and consumed through the **tauriClient** **Runtime Boundary**.
- A **Boundary** owns normalization and validation so **Contract Truth** does not leak into scattered UI callsites.
- The **Product Spine** moves from **Product Intent** through **UI State**, **IPC Contract**, **Backend Lifecycle**, and **Artifact Truth**.
- The **Job Registry** is the authority for **Processing Flow** lifecycle, queue state, and cancellation.
- A **Terminal Outcome** contributes to **Terminal Truth** only after backend processing resolves the job's final state.
- A **Metadata Intent Patch** is compiled at the **Runtime Boundary** and preserved across the **IPC Contract** so clear intent is never inferred from sentinel values.
- A **Fallback** must appear in the **Fallback Register** and stay observable until it is removed or renewed.
- The **Standard Gate** and focused **UI Workflow Smoke Test** coverage are proof surfaces for keeping **Contract Truth** and **Operational Truthfulness** honest.
- A **Task Spec** is a **Durable Workflow Surface** for substantial work produced through **decision-alignment**; it complements, but does not replace, canon repo docs.
- A **Grey-Box Module** publishes a **Public API Strip** and hides a **Private Cluster** behind it; only one **Module Owner** holds any given product rule.
- A **Reach-Through** is the diagnostic for an **Ownership Smear**; a **Boundary Assertion** is the script-enforced cure.
- Each **Public API Strip** in the **Five Public APIs** set is locked by **Contract Tests**; internal cluster changes are safe when contract tests stay green.
- A **Cluster Audit** inspects shape inside a **Private Cluster** without changing the **Public API Strip**; it informs future internal refactor decisions and does not, by itself, change behavior.

## Example Dialogue

> **Dev:** "Should the UI call the generated command directly when saving metadata?"
> **Domain expert:** "No. Route it through the **tauriClient** so the **Metadata Intent Patch** stays explicit at the **Runtime Boundary** and **Contract Truth** remains centralized."

## Flagged Ambiguities

- "API", "command surface", and "runtime boundary" can blur together. Prefer **IPC Contract** for the Rust-exported command/event set and **Runtime Boundary** for the handwritten TS adapter seam.
- "generated bindings" can sound like the primary client. Prefer **Generated Bindings** for drift detection and typed integration, and **tauriClient** for the real runtime adapter.
- "preview" can mean different things. Prefer **Output Path Preview** for path derivation, **Processing Preflight Plan** for pre-run backend review, **audio preview** for a short media render, and **preview artifact** for the file created by that render.
- "fallback" can drift into "temporary workaround." Prefer **Fallback** only for registered, observable, sunset-bound behavior; otherwise call it a compatibility path, recovery default, bug, or design decision.
- "metadata" and "metadata intent" are not interchangeable. Prefer **Metadata Intent Patch** when the important question is user intent to set, clear, or preserve a field; use metadata draft, write plan, lookup result, or tag projection when those narrower concepts are meant.
- "gate", "check", "test", and "smoke test" are different confidence shapes. Prefer **Standard Gate** for `scripts/checks.sh standard`, **Boundary Assertion** for repo scripts that block broken imports or policy drift, **Contract Test** for public API behavior, and **UI Workflow Smoke Test** for deterministic user-flow proof.
- Product names and implementation names should not blur together. For example, **Book Binder** is user-facing product language; **Merge** is the processing job type.
- "minimal churn" can be mistaken for "smallest diff." Prefer **Minimal Churn** as fewer correction loops and less rework, even when the better fix is somewhat broader.
- "status", "progress", and "terminal outcome" are not interchangeable. Prefer **Terminal Outcome** only for final per-job status and **Terminal Truth** for the backend-owned final report.
- "module" alone is ambiguous in ABB. Prefer **Grey-Box Module** for the owned Public-API + Private-Cluster unit, and **Private Cluster File** for any implementation file inside one.
- "API" can mean three different things in ABB. Prefer **IPC Contract** for the Rust-declared command/event set, **Runtime Boundary** for the TS adapter (`tauriClient`), and **Public API Strip** for the externally allowed import set of any **Grey-Box Module**.
- "contract" can be ambiguous. Prefer **IPC Contract** for the cross-language command/event set and **Contract Test** for the behavior-locked test of any **Grey-Box Module's** Public API Strip.
- "refactor" can mean either internal cluster reshape or boundary change. Prefer "**Cluster Audit** + internal refactor" for changes inside a Private Cluster (no Public API Strip impact) and "**Public API Strip** change" for anything callers will notice.
