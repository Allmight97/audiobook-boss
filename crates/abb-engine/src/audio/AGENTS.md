# Audio Pipeline Directives

This file owns audio integrity rules that cross import discovery, stream
probing, decoder setup, resampling, sample buffering, encoder setup, muxing,
and output validation. `processor/AGENTS.md` owns execution-stage rules.

## Public API Strip

- Audio is the Audio Engine Deep Module. Import from `crate::audio`; the strip
  is the `pub use` list in `audio/mod.rs`. Child modules (`processor`,
  `settings_encoder`, `path_validation`, `cleanup`) stay private.
- `validate_resolved_audio_inputs` stays processor-private. Preflight and
  execution validate through the resolved audio plan.
- `AudioExecutionRequest` takes encoder settings from the processing context,
  so it cannot carry conflicting copies. Inspection owns source preservation
  capability; the title planner owns the recommendation.
- `EncoderSettingsCapabilities.encoder_configurations` is the single
  per-encoder capability array. Quality presets are backend-owned capability
  values. The FAAC encoder owns upstream profile resolution and opened
  configuration readback.
- Native and bundled FAAC target bitrates check their resolved AAC ceilings
  during preflight and encoder setup. Resolve the encoder once per validation
  call; each title's ceiling uses its combined channels and first input rate.
  Opened NMR and FAAC settings must match the request. FAAC preflight uses the
  same open/readback as execution, including upstream bitrate clamps.
- Linkage is not file compatibility: per-file trial decoding selects the
  decoder (`processor/streams.rs` tests the linked AAC decoders).

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
- User-facing diagnostics use sanitized display strings. Started jobs record
  ordered source paths at info level, with debug escaping so same-named files
  stay distinguishable.
- Lossy strings serve display ordering only, never identity or command argv.

## Hard Invariants

- For planar audio, use typed plane access such as `frame.plane::<T>(ch)` and
  `frame.plane_mut::<T>(ch)`, or an explicit FFmpeg `extended_data`-aware
  helper. `data(ch)` and `data_mut(ch)` are not channel-presence or sample-copy
  truth for planar audio.
- Byte `linesize` is not per-channel audio truth for planar frames. A zero byte
  linesize on channel index `> 0` needs typed-plane or raw-plane confirmation
  before it counts as a missing channel.
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
- Encoder option changes carry evidence for the affected encoder path:
  targeted tests, real-file `ffprobe`/`ffmpeg` diagnostics, or documented
  external encoder behavior.
- Sample-buffer, resampler, or encoder-boundary changes include regression
  coverage for channel preservation and tail/frame behavior. Native AAC changes
  include a real-media probe when feasible: codec/profile, sample rate,
  channels, duration, and a channel-level check such as RMS/peak parity. Notes
  on Native AAC artifacts separate structural correctness from subjective
  encoder quality.

## Audio Integrity Traps

Each of these signals an audio-boundary assumption to investigate:

- repeated frame-plane warnings
- preview/full divergence for the same encoder path
- missing or silent output channels
- distorted audio with structurally normal progress reporting
- mismatched source/output duration beyond expected preview boundaries
- wrapper API behavior that disagrees with FFmpeg frame/layout semantics

When one appears, name the affected boundary, state the assumption used to
continue, and add or propose the smallest regression test, invariant, or doc
guard that prevents recurrence.

## Chapter Intake

Analysis attaches CUE diagnostics and a source-fingerprinted candidate chapter
plan to each MP3. `apply_chapter_plans` validates accepted plans against the
audio identity and duration before dispatch. CUE confirmation or Ignore is
explicit; multi-source CUE merging is rejected. Encoders consume accepted
chapters. Preview omits chapters.

## Title Audio Plan

`output_plan` resolves one output format and audio intent per title. M4B/AAC is
the default; MP3 is packet pass-through; Opus supports M4A (MP4 muxer) and MKA.
Auto copies joinable, matching audio when every source has a known bitrate at or
below `COMPACT_AUDIO_MAX_BITRATE`; otherwise it plans encoding. Explicit Preserve
copies regardless of bitrate and always copies. MP3 output stays copy-only
regardless of bitrate: it has no encoder route and rejects incompatible copy
boundaries. Strict source/packet validation applies to every copy. Default M4B
encoding uses the built-in AAC defaults (NMR 65 kbps target, source channels and
automatic rate), independent of saved user preferences. Explicit Encode applies
the settings.

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
