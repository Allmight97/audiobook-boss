# abb-ffmpeg

- This layer offers operations (open, read, copy a stream, write a header).
  Which streams to keep, which options a format needs, and every other product
  rule stay in `abb-engine`.
- A codec, sample format, or profile stays FFmpeg's number (`CodecId`,
  `SampleFormat`, `profile`), so a value this build does not name is still a
  value.
- A new operation gets a failure-path test: typed error, no file left. Check a
  new owning type with macOS `leaks --atExit` on the test binary.
