# Audio Processor Directives

Owns stage orchestration, cancellation checkpoints, and cleanup guarantees for
the execution pipeline in this directory. This is the private execution cluster
of the Audio Engine Deep Module; code outside `audio/` uses `crate::audio`.
Final artifact commit policy lives in `output_artifact/`; finalization delegates
those decisions to it.

## Preferred Path

- Keep stage flow explicit: prepare -> execute -> finalize. Emit stage-aligned
  progress and failure states, so UI status reflects real backend state.
- Encoding preparation validates every source against its retained inspection
  fingerprint before creating a workspace. A queued replacement fails instead
  of running with an audio plan resolved for the previous file.
- Preserve dispatch happens before encoder resolution. Its blocking worker
  copies an eligible source into the tracked workspace, applies planned metadata
  intent through the metadata owner, and uses shared finalization. Copy
  cancellation stays typed; source bytes are never the metadata writer target.
  Revalidate the source path at the copy boundary after any scheduler wait;
  cached import validity does not authorize reopening a replaced path. Compare
  the inspected size/modified-time fingerprint against the opened source handle
  before creating the staged copy.
- `preserve_merge` owns compatibility and packet-copy joining of AAC or MP3
  titles: matching configuration/rate/channels/time base, complete contiguous
  frames, and no per-source priming/trimming. Unsupported joins fail explicitly
  without falling back to an encoder. Preflight scans timing; execution repeats
  the check against fingerprint-validated private source copies. Metadata and
  chapter finalization and output publication use their existing owners.
- Resolve the encoder once at dispatch (`resolution.rs`) and carry that choice
  into encoder setup. Explicit Native, Apple, and Opus selections validate their
  linked encoder; bundled FAAC is always available. Auto resolves to Native NMR
  and does not select Apple or FAAC.
- Use app-cache local processing workspaces, cleanup guards, and deterministic
  teardown for temp artifacts. Finalize completes filesystem operations before
  success is reported.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns
  final artifact truth. Final artifact `rename`, `copy`, and `hard_link` live in
  `output_artifact`.
- Every route uses the shared final artifact commit handoff. Route-specific code
  stages media locally; final commit and success wording stay centralized.
  MP4-family tag truth: `metadata/AGENTS.md`.
- Preview artifacts omit chapters until a product decision wires real chapter
  emission and proves it against actual artifact metadata.
- Drop probe/inspection contexts before reopening the same path for decoder
  trials, processing, replacement, or another library.
- Check cancellation at critical boundaries, including post-move and
  pre-success paths.
- Register newly created workspaces with cleanup before observing cancellation.
  Failed cleanup stays tracked for retry; post-publication cleanup failure
  preserves success with an explicit warning from Output Artifact.
- Metadata finalize writes happen only for supported container paths and
  validated metadata payloads.

## Encoder diagnostics

- Shared encoder run records use the private `run_diagnostics` helper for file
  writes, requested settings, and monotonic/wall-clock timing. Records take one
  write lock and append to the file `ABB_ENCODING_LOG` names. Unset or empty
  disables the records. Unavailable opened settings stay explicitly `unknown`.
- Finalization emits `audio_output` from a read-only probe of the completed
  staged file, before publication. These are observed file properties, not
  encoder configuration; unavailable diagnostics never change processing
  success.
- The private `encoder::EncoderSession` owns the selected backend, PCM
  submission, packet muxing, drain, and trailer. Callers submit contiguous
  frames and finish the session; backend handles and packet mechanics stay
  inside `encoder/`. Declare its output cleanup guard before opening the session
  so handles close before cleanup removes a failed output.

## FAAC and HE-AAC file timing

- `encoder/faac.rs` owns requested parameters and resolved configuration. Its
  opened profile determines frame size, mux profile, priming, postroll policy,
  and encoding-tool tag; profile Auto must work for both LC and HE.
- `he_timing` owns the decoded PCM interval of HE-AAC MP4 inputs. ABB's
  FAAC HE files are recognized by their encoding-tool tag; keep each recognized
  convention (2079, 2080) when upgrading upstream priming. LC uses a distinct
  tag and its returned encoder delay. A third-party HE or HE v2 file gets a
  window only when it declares iTunSMPB. Apple's encoder and upstream's `faac`
  frontend both leave the SBR delay out of that priming, and FFmpeg's native HE
  decoder emits it untrimmed, so the window adds 962. HE files with only an
  edit list, or with no gapless metadata (FFmpeg's muxer with Apple's encoder),
  keep FFmpeg's trimming; nothing in them says which convention they follow.
- A windowed input reads all access units (`ignore_editlist`), because an edit
  list can drop the final unit that holds the delayed tail. The window trims
  at source sample rate before preview, resampling, or concatenation, through
  packet skip metadata that replaces what FFmpeg derived from iTunSMPB.
  Encoder selection does not change the source's playable audio.
- Packet-copy joins refuse only ABB's FAAC HE files (`is_abb_faac_he`); a
  third-party HE file with iTunSMPB stays preservable as before.
