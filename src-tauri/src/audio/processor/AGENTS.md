# Audio Processor Directives

## Scope

- Owns execution pipeline behavior in `src-tauri/src/audio/processor/`.
- Source of truth for processor stage orchestration, cancellation checkpoints, and cleanup guarantees.
- Output artifact commit policy lives in `src-tauri/src/audio/output_path/`; processor finalization delegates final artifact decisions to that boundary.

## Preferred Path

- Keep stage flow explicit: prepare -> execute -> finalize.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use cleanup guards and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns final artifact truth.
- Processor code must not directly perform final artifact `rename`, `copy`, or `hard_link`; `scripts/check-no-bridge-imports.sh` enforces this.
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
- Finalize behavior preserves output integrity through deterministic staging, delegated commit, and cleanup paths.
