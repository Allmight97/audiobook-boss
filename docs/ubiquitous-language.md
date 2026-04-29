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
| **Preflight Processing Plan** | A backend-generated preview of how a processing request will execute before the actual long-running job begins. | dry guess, UI-only estimate |
| **Output Naming Preview** | The backend-owned computation of the intended output path before collision suffixing or final processing writes. | frontend filename guess |
| **Fallback** | A deliberately registered temporary compatibility or integrity path with a concrete trigger, observable signal, and sunset condition. | convenience workaround, silent compatibility shim |
| **Fallback Register** | The canonical list of still-active fallbacks that repo checks enforce and sunset-date. | misc exceptions, hidden legacy notes |

## Verification And Planning
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Standard Gate** | The main non-doc quality gate run via `scripts/checks.sh standard` to validate code, contracts, and repo health. | test pass, quick smoke check |
| **UI Workflow Smoke Test** | A deterministic app-level test that exercises one high-value user path with mocked Tauri boundaries and asserts visible state transitions. | retired scenario verifier, image-only proof |
| **Task Spec** | The one active `docs/specs/<task>.md` working document for substantial multi-session or multi-lane execution. | scratchpad, session log |
| **Source Of Truth** | The owning code or canon surface that should settle a question before implementation claims are made. | memory, assumption, third-party critique |
| **Minimal Churn** | Fewer reactive correction loops and less avoidable rework, not automatically the smallest diff. | smallest patch, least code motion |

## System Ownership Terms
| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Boundary Ownership** | The practice of giving one layer clear responsibility for validation, normalization, and shape translation so logic does not scatter. | shared everywhere, whoever touches it |
| **Contract Truth** | The discipline of making code, generated artifacts, docs, and tests tell the same story about a cross-layer interface. | close enough, implied compatibility |
| **Operational Truthfulness** | The requirement that progress, queue state, failures, and user-visible status reflect what the system is actually doing. | optimistic UI, silent failure |
| **Durable Workflow Surface** | A repo artifact that lets a future agent or engineer resume work without depending on chat memory or guesswork. | transcript archaeology, informal notes |

## Relationships

- The **IPC Contract** is exported from Rust, materialized as **Generated Bindings**, and consumed through the **tauriClient** **Runtime Boundary**.
- A **Boundary** owns normalization and validation so **Contract Truth** does not leak into scattered UI callsites.
- The **Product Spine** moves from **Product Intent** through **UI State**, **IPC Contract**, **Backend Lifecycle**, and **Artifact Truth**.
- The **Job Registry** is the authority for **Processing Flow** lifecycle, queue state, and cancellation.
- A **Terminal Outcome** contributes to **Terminal Truth** only after backend processing resolves the job's final state.
- A **Metadata Intent Patch** is compiled at the **Runtime Boundary** and preserved across the **IPC Contract** so clear intent is never inferred from sentinel values.
- A **Fallback** must appear in the **Fallback Register** and stay observable until it is removed or renewed.
- The **Standard Gate** and focused **UI Workflow Smoke Test** coverage are proof surfaces for keeping **Contract Truth** and **Operational Truthfulness** honest.
- A **Task Spec** is a **Durable Workflow Surface** for substantial work; it complements, but does not replace, canon repo docs.

## Example Dialogue

> **Dev:** "Should the UI call the generated command directly when saving metadata?"
> **Domain expert:** "No. Route it through the **tauriClient** so the **Metadata Intent Patch** stays explicit at the **Runtime Boundary** and **Contract Truth** remains centralized."

## Flagged Ambiguities

- "API", "command surface", and "runtime boundary" can blur together. Prefer **IPC Contract** for the Rust-exported command/event set and **Runtime Boundary** for the handwritten TS adapter seam.
- "generated bindings" can sound like the primary client. Prefer **Generated Bindings** for drift detection and typed integration, and **tauriClient** for the real runtime adapter.
- "fallback" can drift into "temporary workaround." Prefer **Fallback** only for registered, observable, sunset-bound behavior; otherwise call it a bug, compatibility requirement, or design decision.
- "metadata" and "metadata intent" are not interchangeable. Prefer **Metadata Intent Patch** when the important question is user intent to set, clear, or preserve a field.
- "minimal churn" can be mistaken for "smallest diff." Prefer **Minimal Churn** as fewer correction loops and less rework, even when the better fix is somewhat broader.
- "status", "progress", and "terminal outcome" are not interchangeable. Prefer **Terminal Outcome** only for final per-job status and **Terminal Truth** for the backend-owned final report.
