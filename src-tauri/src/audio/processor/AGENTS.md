# Audio Processor Directives

## Scope

- Owns execution pipeline behavior in `src-tauri/src/audio/processor/`.
- Source of truth for processor stage orchestration, cancellation checkpoints, and cleanup guarantees.
- Output artifact commit policy lives in `src-tauri/src/output_artifact/`; processor finalization delegates final artifact decisions to that boundary.
- This directory is the Audio Engine Deep Module private execution cluster. Code
  outside `src-tauri/src/audio/` must use the parent `crate::audio` public API
  rather than importing processor files directly.
- Private processor tests that assert execution-cluster invariants may live in
  sibling `*_tests.rs` files declared from the owning module with `#[cfg(test)]`
  and `#[path = "..._tests.rs"]`. Do not move private execution tests to a
  separate integration-test directory if that would require making processor
  internals public.

## Preferred Path

- Keep stage flow explicit: prepare -> execute -> finalize.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use app-cache local processing workspaces, cleanup guards, and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.
- Keep external FDK internals split by private mechanism under
  `external_fdk/`; callers should only use the adapter entrypoint.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns final artifact truth.
- Processor code must not directly perform final artifact `rename`, `copy`, or
  `hard_link`; final artifact commit truth lives in `output_artifact`.
- External FDK and native engine paths must use the shared final artifact commit
  handoff. Adapter-specific code may stage media locally, but final artifact
  commit and success wording remain centralized. MP4-family tag truth must route
  through the mp4ameta metadata writer, not a bare remux — the mov muxer
  silently drops non-native tag keys.
- Preview artifacts intentionally omit chapter passthrough/preview chapters
  unless a future product decision wires real chapter emission and proves it
  against actual artifact metadata.
- Drop probe/inspection contexts before reopening the same path for decoder trials, processing, replacement, or another library.
- Cancellation is checked at critical boundaries, including post-move and pre-success paths.
- Terminal paths clean app-owned temporary resources to avoid residue across retries.
- Metadata finalize writes occur only for supported container paths and validated metadata payloads.

## Done Criteria

- Pipeline stages remain explicit and user-visible progress is truthful.
- Cancellation and cleanup semantics remain deterministic.
- Finalize behavior preserves output integrity through deterministic staging, delegated commit, and cleanup paths.
