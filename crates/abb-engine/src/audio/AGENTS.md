# Audio Pipeline Directives

This file owns audio integrity rules that cross import discovery, stream
probing, decoder setup, resampling, sample buffering, encoder setup, muxing,
and output validation. `processor/AGENTS.md` owns execution-stage rules.

## Public API Strip

- Callers import from `crate::audio`; child modules are private, so the
  compiler enforces it.
- Inspection owns source preservation capability and codec family
  (`AudioPreservation`); the title planner owns the recommendation and reads
  both without opening the file.
- Per-encoder bitrate, mode, rate, and FAAC-profile facts live in
  `EncoderSettingsCapabilities.encoder_configurations`; FAAC quality presets
  and Native speed max are top-level capability fields. The FAAC encoder owns
  upstream profile resolution and opened configuration readback.
- Native and bundled FAAC target bitrates check their resolved AAC ceilings
  during preflight and encoder setup. Resolve the encoder once per validation
  call; each title's ceiling uses its combined channels and first input rate.
  Opened NMR and FAAC settings must match the request. FAAC preflight uses the
  same open/readback as execution, including upstream bitrate clamps.
- Linkage is not file compatibility: per-file trial decoding selects the
  decoder (`processor/streams.rs` tests the linked AAC decoders).
  The `AacDecoder` setting set to FAAD3 replaces that choice with bundled FAAD3
  for AAC-LC, HE-AAC, HE-AAC v2, and ADTS inputs; other AAC profiles (USAC,
  Main, LD, ELD) keep the automatic choice. A file sent to FAAD3 that it
  cannot decode fails instead of falling back, so FAAD3 defects stay visible.
  An import reads the setting when it runs. An export or preview records it
  when accepted and keeps it through collision review (`ProcessPayload`,
  `ProcessingContext`), so its inspection and decoding agree. Audible's
  post-download check uses Auto: it proves the file is audio, and the import
  that follows applies the setting. `processor/streams.rs` selects the
  decoder, `decoder.rs` puts both backends behind one interface, and
  `faad_decoder.rs` adapts FAAD3.
- `probe_builtin_audio_decoder` is that probe with FFmpeg's built-in decoder
  only. Audible's decrypt runs it on the partial before committing, so a
  wrong AAXC key fails on every host; import may still try `aac_at`.

## Encoder Routes

- Keep Native AAC, Apple AAC/AAC-AT, bundled FAAC, and Opus differences inside
  this module unless a caller needs a stable capability fact.
- Encoder Auto resolves to Native NMR; Apple and FAAC stay explicit choices.
  Native AAC uses NMR with upstream psychoacoustic defaults and explicit CBR
  rate control, target bitrate, and search speed. FAAC offers profile
  Auto/LC/HE and ABR/VBR and resolves its profile once at open from output
  settings.
- Auto adapts an unsupported bitrate mode to the resolved encoder default;
  explicit encoder requests reject incompatible modes.
- Resolve Auto channels once at the Audio execution boundary: all valid inputs
  mono -> Mono; any stereo input -> Stereo. Multichannel or unknown input
  counts need an explicit Mono/Stereo choice during preflight. Encoders receive
  resolved channels.
- Downmix coefficients are normalized so coherent channels cannot overflow.
  The standard downmix retains center/surround channels and omits LFE.

## Path Display Policy

- Filesystem and process identity uses `Path`, `OsStr`, or `OsString`.
- User-facing diagnostics use sanitized display strings. The import notice is
  the exception: it lists each skipped path in full, because a bare name
  cannot tell same-named files in different folders apart. Started jobs record
  ordered source paths at info level, with debug escaping so same-named files
  stay distinguishable.
- Lossy strings serve display ordering only, never identity or command argv.

## Hard Invariants

- Read and write samples through `abb_ffmpeg::AudioFrame`: `plane_f32` and
  `plane_f32_mut` for planar float, `packed_bytes` and `packed_bytes_mut` for
  packed formats. Each refuses a plane the frame's format lacks, and the layer
  offers no raw plane or `linesize` access. Why: a planar frame's first plane
  and its byte `linesize` say nothing about the other channels.
- Silence padding serves a deliberate short-frame/tail policy or a verified
  missing-plane condition, with regression coverage.
- Sample sanitization repairs NaN/Inf or clamps out-of-range floats before
  encoding; channel-layout, frame-size, and format mismatches stay visible.
  `buffer.rs` owns the log threshold: NaN/Inf always warn; clamped peaks up to
  `CLIP_WARN_PEAK` (+1 dBFS) log at debug, larger excursions warn. The dev-log
  summary counts WARN lines as actionable, so keep the threshold in the log
  level rather than in `scripts/dev-log-analysis.ts`.
- Resampler output buffers account for pending swr delay plus input samples
  scaled to the output rate; EOF drains stream flushed frames through the
  accumulator/encoder instead of collecting the whole drain.
- Sample-buffer, resampler, or encoder-boundary changes include regression
  coverage for channel preservation and tail/frame behavior. Native AAC changes
  include a real-media probe when feasible: codec/profile, sample rate,
  channels, duration, and a channel-level check such as RMS/peak parity. Notes
  on Native AAC artifacts separate structural correctness from subjective
  encoder quality.
- `write_output_header` is the only header writer (`check-known-mistakes.ts`).
  It pins an MP4's movie timescale to the audio sample rate, or a divisor of
  it for a chapter, or a gap between chapter starts, over about 13.5 hours,
  and rejects options the muxer does not take. Why: FFmpeg's own choice drops chapters longer than about 8
  minutes when a cover is present; the mechanism is in its rustdoc.

## Import Discovery

`discover_audio_import_paths` never fails a whole import: each path it cannot
read (dropped path, subfolder, entry, or file that fails
`validate_input_audio_path`) is returned in `skipped` with its error, and the
walk continues. Unsupported extensions and nested symlinks are left out
silently. The session decides what a skip means for the user.

The bundled FFmpeg compiles only the components ABB's formats need, so a file
whose content is another format fails to open even under a supported
extension. A change to `SUPPORTED_AUDIO_FORMATS` extends `ABB_COMPONENTS` in
`vendor/ffmpeg-sys-next-9.0.1/build.rs` in the same change, and gives each new
component its reason in the Enabled components table of that crate's
`ABB-PROVENANCE.md` (`scripts/ffmpeg-upstream.test.ts` fails when the two
differ).

A file's duration is the length the selected decoder yields. When FFmpeg
only estimates it from a bitrate (`Input::duration_is_estimated`: an MP3 without a
length header, joined MP3 parts, raw ADTS, Matroska without a Duration),
analysis measures it (`processor::measured_length_seconds`): packet durations
less trimmed samples where FFmpeg decodes and the time base counts whole
samples, otherwise a full decode. A read error fails the file. Why: a merge
starts each later chapter at the earlier files' durations.

## Chapter Intake

Analysis gives each valid file a source-fingerprinted chapter plan from its
repaired embedded chapters (`metadata/AGENTS.md`). An MP3 also gets CUE
diagnostics and, when its sheet reads, a CUE plan. A file whose chapters
cannot be repaired is listed invalid with the reason; the rest of the import
continues. `apply_chapter_plans` validates accepted plans against the audio
identity and duration before dispatch. CUE confirmation or Ignore is
explicit. `FileListInfo::check_cue_chapters` is the one rule for a title's
sources: a CUE awaiting review, or CUE chapters on a merged title, is
refused; session submission and the title plan call it, and execution does
not repeat it. Encoders consume accepted chapters.

## Title Audio Plan

`output_plan` resolves one output format and audio intent per title. M4B/AAC is
the default; Opus supports M4A (MP4 muxer) and MKA. MP3 is an input only: an
MP3 source always converts, and Explicit Preserve refuses it.
Auto copies joinable, matching audio when every source has a known bitrate at or
below `COMPACT_AUDIO_MAX_BITRATE`; otherwise it plans encoding. Explicit Preserve
always copies. Strict source/packet validation applies to every copy. Default M4B
encoding uses the built-in AAC defaults (NMR 65 kbps target, source channels and
automatic rate), independent of saved user preferences. Explicit Encode applies
the settings.

`format.rs` is the one table of output formats: extension, muxer, codec
family, and allowed encoders. It holds `is_mp4_family_format`, the one test
for an FFmpeg format name in the MP4 family, and `muxer_for_path`, the muxer
every output is created with. Read a format fact there; never list it again. A format "keeps" a source when the source's codec family equals
the format's (`formats_that_keep`). Whether the source can be copied is a
separate fact (`assess_preservation`, `can_preserve`) that `formats_that_keep`
does not read.

Session title plans (`session/plans.rs`) and processing preflight share this
resolver. Only encoding plans resolve encoder availability, channels, and input
rate. Execution receives that resolved plan and takes no frontend preferences.

Auto keeps supported source rates and otherwise rounds upward to the next
encoder/profile rate, capped at its maximum. Opus caps at 48 kHz; explicit FAAC
HE starts at 32 kHz. Opus headers and skip counts use the 48 kHz decoder clock
even with 24 kHz PCM input. Packet-copy Opus stacks need decoder resets that
this join path cannot express, so Auto encodes them. Single Opus files can
remux between supported containers without changing compressed packets.
Container timing for Opus remux: `metadata/AGENTS.md`.
