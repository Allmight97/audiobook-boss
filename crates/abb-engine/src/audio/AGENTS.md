# Audio

- A linked decoder is not proof it decodes a file, so per-file trial decoding
  selects the decoder (`processor/streams.rs`).
- A file the `AacDecoder` setting sends to FAAD3 that FAAD3 cannot decode
  fails instead of falling back, so FAAD3 defects stay visible.
- An export or preview records the decoder setting at acceptance and keeps it
  through review (`ProcessPayload`, `ProcessingContext`), so its inspection
  and decoding agree. Audible's post-download check uses Auto: it only proves
  the file is audio.
- Audible decrypt probes with FFmpeg's built-in decoder
  (`probe_builtin_audio_decoder`), so a wrong AAXC key fails on every host;
  `aac_at` exists only on macOS.
- Keep Native AAC, Apple AAC, bundled FAAC, and Opus differences inside this
  module unless a caller needs a stable capability fact.
- Filesystem and process identity uses `Path`, `OsStr`, or `OsString`; a lossy
  string serves display ordering only, never identity or command argv.
- The import skip notice lists each skipped path in full, because a bare name
  cannot tell same-named files in different folders apart. Other user-facing
  diagnostics use sanitized display strings.
- Read and write samples only through the `abb_ffmpeg::AudioFrame` accessors,
  because a planar frame's first plane and its byte `linesize` say nothing
  about the other channels.
- The resampler keeps swresample's default downmix matrix, which retains the
  center and surround channels and omits LFE; `resampler_options` only
  normalizes the coefficients so coherent channels cannot overflow.
- Silence padding serves a deliberate short-frame or tail policy, or a
  verified missing-plane condition, with regression coverage.
- `scripts/dev-log-analysis.ts` counts WARN lines as actionable, so the clip
  threshold (`CLIP_WARN_PEAK` in `buffer.rs`) lives in the log level, not in
  that script. Clamped peaks up to +1 dBFS are routine and inaudible after a
  lossy decode or a resample of a master normalized near 0 dBFS; a larger
  excursion suggests a gain, format, or layout mismatch.
- A sample-buffer, resampler, or encoder-boundary change adds regression
  coverage for channel preservation and tail and frame behavior. A Native AAC
  change adds a real-media probe when feasible (codec, profile, rate,
  channels, duration, RMS or peak parity per channel), and notes separate
  structural correctness from subjective encoder quality.
- `write_output_header` pins the MP4 movie timescale, because FFmpeg's own
  choice drops chapters longer than about 8 minutes when a cover is present.
  The chapter track counts durations in 32 bits of movie ticks, and FFmpeg
  rejects a delta of `i32::MAX` and rounds each start, so a longer chapter
  or gap takes a divisor of the sample rate and stays two ticks below it.
  Apple Books reads the QuickTime `text` chapter track, not Nero `chpl`, and
  FFmpeg's chapter API sees only `chpl`, so a chapter proof walks the `text`
  track (`../test_cases/media_fixtures.rs`).
- The bundled FFmpeg compiles only the components ABB's formats need, so a
  file whose content is another format fails to open even under a supported
  extension. A new import format needs its fixture recipe in
  `../test_cases/integration_media_import_formats_tests.rs` and its components
  in `vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md`.
- When FFmpeg only estimates a duration from a bitrate
  (`Input::duration_is_estimated`), analysis measures it, because a merge
  starts each later chapter at the earlier files' durations.
- Read an output format fact (extension, muxer, codec family, encoders) from
  `format.rs`; do not list formats again.
- Session title plans and processing preflight share one resolver; execution
  takes that resolved plan and no frontend preferences.
- Opus caps at 48 kHz and its headers and skip counts use the 48 kHz decoder
  clock even with 24 kHz input; libopus counts its trims at its input rate, so
  `processor/encoder/write.rs` converts them. Packet-copy Opus stacks need
  decoder resets this join path cannot express, so Auto encodes them.

## Outside facts

- `ffmpeg_log.rs`: FFmpeg's own log stays at Warning, because at Info the MP4
  demuxer prints an AAX file's checksum, which public tables map to the
  account's activation bytes.
- `file_list.rs`: FFmpeg's MP4 demuxer reports ordinary end of file at a
  missing packet, so import checks that every indexed packet fits in the file.
- `format.rs`: FFmpeg's `format().name()` is a muxer name or a demuxer's
  comma-separated aliases (`mov,mp4,m4a,3gp,3g2,mj2`).
- `processor/decoder.rs`: WAV and PCM open with an unspecified channel layout
  while their frames carry the default layout, and swresample then rejects
  every frame with "Input changed", so the decoder declares the layout.
- `processor/encoder/context.rs`: an encoder that reports frame size 0 takes
  any frame length, but AAC still takes 1024-sample frames. Accepted chapters
  go into the output before its header (#66).
- `processor/encoder/options/apple.rs`: FFmpeg maps `aac_at_quality` to
  Apple's codec quality as `96 - value * 32` (`libavcodec/audiotoolboxenc.c`,
  FFmpeg 9.0), so 0 is the best quality. CVBR needs a target bitrate on the
  context.
- `processor/encoder/options/native.rs`: Native AAC sets `AAC_CODER_NMR` (2,
  `aacenc.h`) and constant rate control (`aac_rc` 0, `aacenc.c`) explicitly,
  because ABB promises a constant target and upstream may change the default.
