# Ubiquitous Language

Compact glossary for ABB terms that change implementation behavior and help align agent with user. Keep this
file small: if a term does not affect ownership, contracts, or verification,
prefer local prose or delete it.

## Runtime Boundary

| Term | Definition | Avoid |
| --- | --- | --- |
| **Runtime Boundary** | The active TS↔Rust seam centered on `tauriClient`; frontend runtime code should not call raw generated commands or raw Tauri `invoke`. | generated seam, raw invoke layer |
| **IPC Contract** | The Rust-declared commands/events in `src-tauri/src/ipc_contract.rs`, exported as generated bindings and adapted by `tauriClient`. | ad hoc API, implicit bridge |
| **Generated Bindings** | Committed generated TypeScript contract output used for drift detection and typed integration, not the handwritten runtime client. | primary client |
| **tauriClient** | Frontend adapter that wraps commands/events/plugins and owns normalization, denormalization, metadata-intent compilation, and error mapping. | direct generated calls |
| **App Settings** | Backend-owned durable preference schema, defaults, validation, JSON storage, and settings IPC commands. | localStorage settings |
| **Runtime Settings Capabilities** | Backend-owned selectable facts for encoder and concurrency controls. UI renders these facts; it does not mirror accept/reject tables. | frontend settings matrix |
| **RemoteSourceRuntime** | Backend-owned provider-neutral remote acquisition: capabilities, backend-only auth, secret vault access, library scan, acquisition jobs, staged files, Supplemental Assets, and purge behavior. | Audible UI helper |

## Workflow Truth

| Term | Definition | Avoid |
| --- | --- | --- |
| **Local Audio Import Boundary** | All local audio ingress after a path exists: picker, folder, drag/drop, and OS Open With route through backend discovery/analysis before FileList append. | frontend allowlist |
| **Remote Source Acquisition** | Provider-shaped ingress that materializes remote titles into ABB-owned staged files before normal local import handoff. | auto-processing |
| **Media Contract** | Backend-neutral media facts, progress, error, and provenance vocabulary that does not expose FFmpeg, AAXClean, provider payloads, or metadata-tool internals. | provider response shape |
| **Supplemental Asset** | Non-audio sidecar acquired with a remote title, currently Supplemental PDF. It never enters audio processing. | audio input |
| **Backend Lifecycle** | Processing sub-owner for operation identity, queue/progress vocabulary, cancellation checks, and terminal-summary truth. | status UI owner |
| **Operation Kind** | Backend-declared operation identity on lifecycle events, currently `processingMerge`, `processingBatch`, or `metadataSave`. | UI-only mode |
| **Operation Result Summary** | Terminal counts for backend operations: total, succeeded, skipped, cancelled, and failed. | UI completion guess |
| **Terminal Outcome** | Final per-job status after backend ambiguity resolves: `success`, `skipped`, `cancelled`, or `failed`. | progress stage |
| **Terminal Truth** | Backend-owned final report used by UI completion state. | optimistic UI |
| **Artifact Truth** | Final files, tags, paths, and terminal results that actually exist after processing or metadata operations complete. | expected output |

## Metadata And Output

| Term | Definition | Avoid |
| --- | --- | --- |
| **Metadata Intent Patch** | Explicit patch-op payload preserving whether a field is set, cleared, or left alone. | raw metadata object |
| **Patch Op** | A metadata action: `set`, `clear`, or `noop`. | magic empty value |
| **Metadata Outcome Plan** | Backend plan that validates/normalizes metadata intent, then produces effective metadata, naming metadata, write instructions, and cover-art policy. | metadata helper chain |
| **Processing Preflight Plan** | Backend preview of how a processing request will execute before the long-running job starts. | dry guess |
| **Output Path Preview** | Backend-owned intended output path before collision suffixing or final writes. | frontend filename guess |
| **Fallback** | Registered temporary compatibility or integrity path with trigger, observable signal, and sunset condition. | silent shim |
| **Fallback Register** | Current fallback rows, triggers, signals, and sunsets in `docs/fallbacks.md`. | hidden legacy notes |

## Ownership

| Term | Definition | Avoid |
| --- | --- | --- |
| **Grey-Box Module** | ABB ownership unit: small Public API Strip, hidden Private Cluster, local rules, narrow checks, and contract tests. | shallow wrapper |
| **Public API Strip** | The symbols callers may use from an owned module. Everything else is private even if technically importable. | everything pub |
| **Private Cluster** | Implementation files behind a Public API Strip. | helper grab bag |
| **Module Owner** | The single module responsible for a product rule or invariant. | shared responsibility |
| **Eight Public APIs** | Current owned API set: Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Outcome Plan, Status Panel Runtime, Audio Engine Deep Module, App Settings, and RemoteSourceRuntime. | generic modules |
| **Reach-Through** | Import or dependency crossing into another module's Private Cluster. Treat as a bug, ownership smear, or contract gap. | shortcut |
| **Contract Test** | Test that pins behavior visible through a Public API Strip. | helper existence test |

## Verification Terms

| Term | Definition | Avoid |
| --- | --- | --- |
| **Core Crate Test** | Focused Rust command selecting an `abb-*-core` package for pure domain logic. | filtered broad-crate test |
| **Media Execution Test** | Currently absent FFmpeg/audio/container lane pending issue #341 reassessment. | default review |
| **UI Workflow Smoke Test** | Deterministic app-level test for one high-value user path with mocked Tauri boundaries and visible state assertions. | screenshot-only check |
| **Active Spec** | Temporary `docs/specs/<task>.md` work packet for substantial multi-session planning or implementation. Delete or distill when done. | permanent feature doc |
| **Minimal Churn** | Fewer correction loops and less avoidable rework, not automatically the smallest diff. | smallest patch |
