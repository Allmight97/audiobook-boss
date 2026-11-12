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
- Silence padding serves a deliberate short-frame or tail policy, or a
  verified missing-plane condition, with regression coverage.
- `scripts/dev-log-analysis.ts` counts WARN lines as actionable, so the clip
  threshold (`CLIP_WARN_PEAK` in `buffer.rs`) lives in the log level, not in
  that script.
- A sample-buffer, resampler, or encoder-boundary change adds regression
  coverage for channel preservation and tail and frame behavior. A Native AAC
  change adds a real-media probe when feasible (codec, profile, rate,
  channels, duration, RMS or peak parity per channel), and notes separate
  structural correctness from subjective encoder quality.
- `write_output_header` pins the MP4 movie timescale, because FFmpeg's own
  choice drops chapters longer than about 8 minutes when a cover is present.
- The bundled FFmpeg compiles only the components ABB's formats need, so a
  file whose content is another format fails to open even under a supported
  extension.
- When FFmpeg only estimates a duration from a bitrate
  (`Input::duration_is_estimated`), analysis measures it, because a merge
  starts each later chapter at the earlier files' durations.
- Read an output format fact (extension, muxer, codec family, encoders) from
  `format.rs`; do not list formats again.
- Session title plans and processing preflight share one resolver; execution
  takes that resolved plan and no frontend preferences.
- Opus caps at 48 kHz and its headers and skip counts use the 48 kHz decoder
  clock even with 24 kHz input. Packet-copy Opus stacks need decoder resets
  this join path cannot express, so Auto encodes them.
