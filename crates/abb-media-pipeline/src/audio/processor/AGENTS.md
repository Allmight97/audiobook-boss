# Audio Processor

- Encoding preparation checks every source against its inspection fingerprint
  before creating a workspace, so a queued replacement fails instead of
  running with a plan resolved for the previous file. A queued job
  re-validates when it runs, because preflight may be long past.
- Preserve revalidates the source path at the copy boundary after any
  scheduler wait and compares the inspected size and modified time against the
  opened handle: cached import validity never authorizes reopening a replaced
  path.
- A join writes the full metadata onto the new file (`WriteFreshMux`) and a
  single copy patches its own tags (`PatchStagedCopy`), because a join has no
  tags to patch and a full write onto a single copy would drop the tags the
  plan never touched.
- Source bytes are never the metadata writer's target; Preserve writes into
  its staged copy.
- Auto sample rate resolves when the title is planned (`audio/output_plan.rs`),
  never at execution.
- Every route stages media locally and hands off to the shared final commit;
  final commit and success wording stay in one place.
- A preview carries the export's metadata without chapters, because the owner
  previews to hear the encode and check the metadata.
- A preview cuts each file at an exact sample (`PreviewState::take`), so single
  and merged titles share one stop rule and tests can assert the length.
- Drop FFmpeg probe, inspection, and remux contexts before reopening the same
  path for decoder trials, processing, replacement, an MP4 tag write, or
  another library.
- Cancel is checked again right before commit, because publishing may first
  wait for the output lock and write an edit, and a title cancelled meanwhile
  must not publish.
- The accumulator flush keeps the real tail length, so encoders mark their own
  padding non-playable instead of turning ABB's zero pad into source audio.
- A user cancel is not logged as ERROR: the job's terminal record reports it,
  and dev-log analysis counts ERROR lines as degradation.
- The pipeline (prepare, encode, finalize) is CPU-bound and runs on a blocking
  thread.
- Declare the job's `Workspace` before opening the `EncoderSession`, so handles
  close before cleanup removes a failed output.

## HE-AAC timing

- ABB's FAAC HE files are recognized by their encoding-tool tag. Each tag
  fixes the priming sample count in the file (`he_timing.rs`: 2079 for the
  original tag, 2080 for `timing-2`), so an upstream priming upgrade gets a new
  tag and keeps both. FAAC LC uses a distinct tag and the encoder delay the
  encoder returns.
- A third-party HE or HE v2 file gets a window only when it declares
  iTunSMPB. Apple's encoder and upstream's `faac` frontend both leave the SBR
  delay out of that priming. A file with only an edit list, or with no gapless
  metadata (FFmpeg's muxer with Apple's encoder), keeps FFmpeg's trimming,
  because nothing in it says which convention it follows.
- FFmpeg's native HE decoder emits the SBR delay untrimmed, so its window adds
  962 samples; FAAD3 trims its own reported delay, so its window holds priming
  alone (`SbrDelayOwner`).
- A windowed input reads all access units (`ignore_editlist`), because an edit
  list can drop the final unit that holds the delayed tail.
- `he_timing.rs` reads iTunSMPB as FFmpeg's MP4 reader does: four hex fields
  of at most 16 digits, counts up to 2^40, priming in 1..16384, a positive
  length.
- An HE file's half-rate timeline cannot express an odd full-rate duration;
  LC and even HE durations keep an exact sample count.
- FAAC HE packets past the audio are kept and marked discard, and the
  crossing packet keeps its full duration (`encoder/write.rs`
  `retain_postroll`), so the HE decoder's postroll stays available.
- A backward-compatible HE AudioSpecificConfig starts with the LC object
  type; only the demuxer's probed profile accounts for the SBR or PS
  extension (`streams.rs`).

## FAAD3 and packet joins

- `faad_decoder.rs` shifts the stream by FAAD's reported delay before FFmpeg's
  packet trimming applies, the order the `faad` frontend uses; the trimming
  mirrors `discard_samples()` in libavcodec's `decode.c`, and its tests use
  FFmpeg's `tests/ref/fate/api-skip-samples` values. FAAD reveals its output
  format (implicit SBR, ADTS) only with its first PCM, so the resampler is
  built after it.
- A packet-copy join takes only AAC sources with complete, contiguous frames
  (`preserve_merge.rs`), because one stream cannot carry interior priming,
  padding, or a decoder reset.
