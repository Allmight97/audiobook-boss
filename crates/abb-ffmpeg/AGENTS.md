# abb-ffmpeg

- This layer offers operations (open, read, copy a stream, write a header).
  Which streams to keep, which options a format needs, and every other product
  rule stay in `abb-engine`.
- A codec, sample format, or profile stays FFmpeg's number (`CodecId`,
  `SampleFormat`, `profile`), so a value this build does not name is still a
  value.
- Text from a file is bytes, not always UTF-8 (`Text`): copy it as bytes and
  convert only for display. `Tags::get` ignores ASCII case in the key, as
  FFmpeg's own lookup does.
- A new operation gets a failure-path test: typed error, no file left. Check a
  new owning type with macOS `leaks --atExit` on the test binary.
