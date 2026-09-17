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
- Preserve dispatch happens before encoder resolution. Its blocking worker
  copies an eligible source into the tracked workspace, applies explicit metadata
  intent through the metadata owner, and uses shared finalization. Copy cancellation
  remains typed; source bytes are never the metadata writer target.
- Resolve the encoder once at adapter dispatch and carry that choice into the
  in-process setup. Explicit Native/Apple selections validate their linked
  encoder without probing external FDK; bundled FAAC is available without an
  external toolchain. Auto still resolves the available toolchain in FDK →
  Apple → Native order and does not select FAAC. Do not repeat
  external-toolchain detection while opening the encoder.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use app-cache local processing workspaces, cleanup guards, and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.
- Keep external FDK internals split by private mechanism under
  `external_fdk/`; callers should only use the adapter entrypoint.
  Its mono output explicitly declares PS absent in the AAC configuration before
  metadata finalization. This applies only to that freshly encoded mono stream;
  stream-copy the compressed packets and keep the corrected output under the
  same workspace cleanup owner.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns final artifact truth.
- Processor code must not directly perform final artifact `rename`, `copy`, or
  `hard_link`; final artifact commit truth lives in `output_artifact`.
- External FDK and in-process engine paths must use the shared final artifact commit
  handoff. Adapter-specific code may stage media locally, but final artifact
  commit and success wording remain centralized. MP4-family tag truth must route
  through the mp4ameta metadata writer, not a bare remux — the mov muxer
  silently drops non-native tag keys.
- Preview artifacts intentionally omit chapter passthrough/preview chapters
  unless a future product decision wires real chapter emission and proves it
  against actual artifact metadata.
- Drop probe/inspection contexts before reopening the same path for decoder trials, processing, replacement, or another library.
- Cancellation is checked at critical boundaries, including post-move and pre-success paths.
- Register newly created workspaces with cleanup before observing cancellation.
  Failed cleanup remains tracked for retry; post-publication cleanup failure
  preserves success with an explicit warning from Output Artifact.
- Metadata finalize writes occur only for supported container paths and validated metadata payloads.

## Encoder diagnostics

- Shared encoder run records use the private `run_diagnostics` helper for requested
  settings and monotonic/wall-clock timing. Adapter records add only facts owned by
  that adapter; unavailable opened settings remain explicitly `unknown`.
- The private `encoder::EncoderSession` owns the selected backend, PCM
  submission, packet muxing, drain, and trailer. Callers submit contiguous
  frames and finish the session; backend handles and packet mechanics stay
  inside `encoder/`.
  Declare its output cleanup guard before opening the session so handles close
  before cleanup removes a failed output. Final publication stays Output Artifact-owned.

## FAAC file timing

- `faac_timing` owns the core priming used in MP4 and the native decoder's PCM
  interval. Its standard encoding-tool tag identifies ABB-produced FAAC files;
  never apply that interval to arbitrary HE-AAC files.
- Re-import reads all FAAC access units, including decoder postroll, and trims
  at source sample rate before preview, resampling, or concatenation. In-process
  packet skip metadata and external FDK filters consume the same interval.
  Encoder selection does not change the source's playable audio.

## Done Criteria

- Pipeline stages remain explicit and user-visible progress is truthful.
- Cancellation and cleanup semantics remain deterministic.
- Finalize behavior preserves output integrity through deterministic staging, delegated commit, and cleanup paths.
