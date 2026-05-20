# Audio Processor Directives

## Scope

- Owns execution pipeline behavior in `src-tauri/src/audio/processor/`.
- Source of truth for processor stage orchestration, cancellation checkpoints, and cleanup guarantees.
- Output artifact commit policy lives in `src-tauri/src/output_artifact/`; processor finalization delegates final artifact decisions to that boundary.
- This directory is the Audio Engine Deep Module private execution cluster. Code
  outside `src-tauri/src/audio/` must use the parent `crate::audio` public strip
  rather than importing processor files directly.
- Private processor tests that assert execution-cluster invariants may live in
  sibling `*_tests.rs` files declared from the owning module with `#[cfg(test)]`
  and `#[path = "..._tests.rs"]`. Do not move private execution tests to
  `src-tauri/tests` if that would require making processor internals public.

## Preferred Path

- Keep stage flow explicit: prepare -> execute -> finalize.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use cleanup guards and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns final artifact truth.
- Processor code must not directly perform final artifact `rename`, `copy`, or `hard_link`; `scripts/check-no-bridge-imports.sh` enforces this.
- External FDK and native engine paths must use the shared finalization handoff;
  adapter-specific code may stage media and write metadata, but final artifact
  commit and success wording remain centralized.
- Drop probe/inspection contexts before reopening the same path for decoder trials, processing, replacement, or another library.
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
