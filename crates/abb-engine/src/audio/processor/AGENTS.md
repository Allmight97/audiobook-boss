# Audio Processor Directives

Owns stage orchestration, cancellation checkpoints, and cleanup for the
execution pipeline. Final artifact commit policy lives in `output_artifact/`.

## Preferred Path

- Encoding preparation validates every source against its retained inspection
  fingerprint before creating a workspace. A queued replacement fails instead
  of running with an audio plan resolved for the previous file.
- Preserve dispatch happens before encoder resolution. Its blocking worker
  copies an eligible source into the tracked workspace, writes its accepted
  chapter plan and planned metadata intent through the metadata owner, verifies
  the chapters, and uses shared finalization. Copy
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
  linked encoder; bundled FAAC is always available.

## Hard Invariants

- Finalization reports success only after the output artifact boundary returns
  final artifact truth. Final artifact moves live in `output_artifact`
  (`check-known-mistakes.ts`).
- Every route uses the shared final artifact commit handoff. Route-specific code
  stages media locally; final commit and success wording stay centralized.
  MP4-family tag truth: `metadata/AGENTS.md`.
- A preview carries the export's metadata without chapters. Why: the owner
  previews to hear the encode and check the metadata; chapter layout is not a
  preview's job.
- A preview takes the start of each file, cut at an exact sample
  (`PreviewState::take`). Why: one stop rule for single and merged titles, and
  a preview length that tests can assert exactly.
- Drop probe/inspection contexts before reopening the same path for decoder
  trials, processing, replacement, or another library.
- Register newly created workspaces with cleanup before observing cancellation.
  Failed cleanup stays tracked for retry; post-publication cleanup failure
  preserves success with an explicit warning from Output Artifact.
- Metadata finalize writes happen only for supported container paths and
  validated metadata payloads.
- Cancel is checked again right before commit: publishing may first wait for
  the output lock and write an edit, and a title cancelled meanwhile must not
  publish. An export title publishes through its output record, which writes
  an edit accepted meanwhile to the staged file first.
- A queued job re-validates its source when it runs; preflight may be long past.
- The accumulator flush keeps the real tail length, so encoders mark their own
  padding non-playable instead of turning ABB's zero pad into source audio.
- A user cancel is not logged as ERROR: the job's terminal record reports it,
  and dev-log analysis counts ERROR lines as degradation.
- The pipeline (prepare, encode, finalize) is CPU-bound and runs on a blocking
  thread.

## Encoder diagnostics

- Shared encoder run records use the private `run_diagnostics` helper for file
  writes, requested settings, and monotonic/wall-clock timing. Records take one
  write lock and append to the file `ABB_ENCODING_LOG` names. Unset or empty
  disables the records. Unavailable opened settings stay explicitly `unknown`.
- Each input's decoder logs `decode_open` and, when it closes,
  `decode_summary` (packets, samples, decode speed; FAAD3 adds its trims and
  concealment counts). The dev-log summary shows both lines.
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
  frontend both leave the SBR delay out of that priming. HE files with only an
  edit list, or with no gapless metadata (FFmpeg's muxer with Apple's encoder),
  keep FFmpeg's trimming; nothing in them says which convention they follow.
- Who removes SBR delay (`SbrDelayOwner`): FFmpeg's native HE decoder emits it
  untrimmed, so the window adds 962; FAAD3 trims its own reported delay, so
  its window holds priming alone.
- A windowed input reads all access units (`ignore_editlist`), because an edit
  list can drop the final unit that holds the delayed tail. The window trims
  at source sample rate before preview, resampling, or concatenation, through
  packet skip metadata that replaces what FFmpeg derived from iTunSMPB.
  Encoder selection does not change the source's playable audio. A metadata
  remux (Preserve with repaired chapters) keeps the tag and that final unit.
- Packet-copy joins refuse ABB's FAAC HE files by tag (`is_abb_faac_he`).
  Other files go through the normal packet checks, which refuse a packet that
  carries trim metadata (a third-party file with iTunSMPB).
