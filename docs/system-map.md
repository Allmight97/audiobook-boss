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
| IPC contract | Command, event, and payload shapes crossing TS <-> Rust. |
| Backend lifecycle | Planning, queueing, execution, cancellation, skipping, and finalization. |
| Artifact truth | Final files, tags, paths, and terminal results on disk. |

## Boundary Rule

UI expresses intent and renders truth. Rust produces durable truth. The IPC
boundary prevents the two from drifting.

## Core Truth Boundaries

- UI code routes runtime commands/events through `src/lib/tauri/client.ts`.
- `tauriClient` adapts generated bindings from `src/lib/generated/tauri.ts`.
- Rust commands are registered in `src-tauri/src/ipc_contract.rs` and implemented under `src-tauri/src/commands/`.
- Processing plans are built before execution and reviewed before jobs run.
- Run orchestration owns dispatch and side effects; terminal outcome helpers own final status normalization.
- Metadata intent is compiled at the TS boundary and preserved through Rust writes and readback.
- Processing adapters produce media artifacts; final results report what actually happened.

## Core Invariants

- Every processing job has exactly one terminal outcome: `success`, `skipped`, `cancelled`, or `failed`.
- UI renders backend terminal truth; it does not invent final status.
- Metadata `set`, `clear`, and `preserve` intent stays distinct across the boundary.
- Path validation remains active for file inputs, output locations, and artifact writes.
- Fallbacks are explicit, observable, registered, and sunset-bound.
- Generated bindings are not hand-edited.

## Task Frame

For meaningful work, identify:

- user/product outcome
- layer touched
- owned truth
- boundaries involved
- invariants protected
- proof of done

Use [docs/api-map.md](api-map.md) for the runtime command/event index and
[docs/ubiquitous-language.md](ubiquitous-language.md) for canonical terms.
