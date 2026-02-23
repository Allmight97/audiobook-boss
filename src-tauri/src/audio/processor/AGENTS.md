# Audio Processor Directives

## Scope

- Owns execution pipeline behavior in `src-tauri/src/audio/processor/`.
- Source of truth for finalize semantics, cancellation checkpoints, and cleanup guarantees.

## Preferred Path

- Keep stage flow explicit: prepare -> execute -> finalize.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use cleanup guards and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.

## Hard Invariants

- Finalization prefers filesystem move semantics and uses copy/replace fallback when move fails.
- Cancellation is checked at critical boundaries, including post-move and pre-success paths.
- Terminal paths clean temporary resources to avoid residue across retries.
- Metadata finalize writes occur only for supported container paths and validated metadata payloads.

## Canary Trigger

- Trigger Canary when processor behavior relies on implicit assumptions across execute/finalize/cancel stages.
- Report hidden assumption, working behavior used, and minimal invariant update proposal.
- Continue unless ambiguity risks false success, file loss, or stuck cleanup.

## Done Criteria

- Pipeline stages remain explicit and user-visible progress is truthful.
- Cancellation and cleanup semantics remain deterministic.
- Finalize behavior preserves output integrity across normal and fallback paths.
