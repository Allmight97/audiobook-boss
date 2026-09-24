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
- Encoding preparation validates every source against its retained inspection
  fingerprint before creating a workspace. A queued replacement must fail rather
  than run with an audio plan resolved for the previous file.
- Preserve dispatch happens before encoder resolution. Its blocking worker
  copies an eligible source into the tracked workspace, applies explicit metadata
  intent through the metadata owner, and uses shared finalization. Copy cancellation
  remains typed; source bytes are never the metadata writer target.
  Revalidate the source path at the copy boundary after any scheduler wait;
  cached import validity does not authorize reopening a replaced path. Compare the
  inspected size/modified-time fingerprint against the opened source handle before
  creating the staged copy.
- `preserve_merge` owns compatibility and packet-copy joining of AAC or MP3 titles:
  matching configuration/rate/channels/time base, complete contiguous frames,
  and no per-source priming/trimming. Unsupported joins fail explicitly without
  falling back to an encoder. Preflight scans timing; execution repeats the
  check against fingerprint-validated private source copies. Metadata/chapter
  finalization and output publication use their existing owners.
- Resolve the encoder once at adapter dispatch and carry that choice into the
  in-process setup. Explicit Native/Apple selections validate their linked
  encoder without probing external FDK; bundled FAAC is available without an
  external toolchain. Auto resolves Native NMR → FDK and does not select Apple or FAAC. Do not repeat
  external-toolchain detection while opening the encoder.
- Emit stage-aligned progress/failure states so UI status reflects real backend state.
- Use app-cache local processing workspaces, cleanup guards, and deterministic teardown for temp artifacts.
- Preserve finalize behavior that completes filesystem operations before success is reported.
- Keep external FDK internals split by private mechanism under
  `external_fdk/`; callers should only use the adapter entrypoint.
  Its HE-AAC v1 mono output explicitly declares PS absent in the AAC configuration before
  metadata finalization. LC bypasses this HE-specific correction. This applies only to that freshly encoded mono stream;
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

- Shared encoder run records use the private `run_diagnostics` helper for file
  writes, requested settings, and monotonic/wall-clock timing. Both routes share
  one write lock and prefer `ABB_ENCODING_LOG`; `ABB_LOG_FILE` is the legacy
  fallback, truncated once per process. Adapter records add only facts owned by
  that adapter; unavailable opened settings remain explicitly `unknown`.
- Finalization emits `audio_output` from a read-only probe of the completed staged
  file, before publication. These are observed file properties, not encoder
  configuration; unavailable diagnostics never change processing success.
- The private `encoder::EncoderSession` owns the selected backend, PCM
  submission, packet muxing, drain, and trailer. Callers submit contiguous
  frames and finish the session; backend handles and packet mechanics stay
  inside `encoder/`.
  Declare its output cleanup guard before opening the session so handles close
  before cleanup removes a failed output. Final publication stays Output Artifact-owned.

## FAAC file timing

- `encoder/faac.rs` owns requested parameters and resolved configuration. Its
  opened profile determines frame size, mux profile, priming, postroll policy,
  and encoding-tool tag; profile Auto must work for both LC and HE.
- `faac_timing` owns HE core priming in MP4 and the native decoder's PCM
  interval. Its encoding-tool tag identifies the timing convention of
  ABB-produced HE files; retain each recognized convention when upgrading
  upstream priming. LC uses a distinct tag and its returned encoder delay.
  Apply the HE interval only to the recognized HE provenance.
- HE re-import reads all FAAC access units, including decoder postroll, and trims
  at source sample rate before preview, resampling, or concatenation. In-process
  packet skip metadata and external FDK filters consume the same interval.
  Encoder selection does not change the source's playable audio.

## Done Criteria

- Pipeline stages remain explicit and user-visible progress is truthful.
- Cancellation and cleanup semantics remain deterministic.
- Finalize behavior preserves output integrity through deterministic staging, delegated commit, and cleanup paths.
