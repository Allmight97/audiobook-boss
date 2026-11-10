# abb-ffmpeg

The only ABB code that calls the FFmpeg C API. The engine uses it with no
`unsafe`, no FFmpeg pointer, and no FFmpeg lifetime.

- The public surface is owned values and typed errors (`Error`, with the
  `Step` that failed). Nothing public takes or returns a raw pointer.
- The layer offers operations (open, read, copy a stream, write a header).
  Which streams to keep, which options a format needs, and every other
  product rule stay in `abb-engine`.
- Each owning type frees what it allocates in `Drop`. An `Output` or `Muxer`
  dropped before `Muxer::finish` succeeds closes and removes its file, so a
  failed or stopped write leaves nothing behind.
- An operation that FFmpeg can only do in order takes the earlier state by
  value: `Output::write_header` consumes the `Output` and returns the `Muxer`
  that writes packets.
- Text from a file is bytes, not always UTF-8 (`Text`). Copy it as bytes;
  convert it only for display, with `Text::display`: valid UTF-8 as UTF-8,
  anything else as Latin-1. `Tags::get` ignores ASCII case in the key, as
  FFmpeg's own lookup does.
- A codec, sample format, or profile is kept as FFmpeg's number (`CodecId`,
  `SampleFormat`, `profile`), so a value this build does not name is still a
  value.
- Error text never holds a path or an option value. `SecretOptions` has no
  `Debug`, and input options FFmpeg did not take come back as names only.
- Every `unsafe` block has a `// SAFETY:` note; `lib.rs` denies
  `clippy::undocumented_unsafe_blocks`.
- `bash scripts/verify.sh engine` runs the crate's tests and clippy. A new
  operation gets a failure-path test: typed error, no file left.
  Check a new owning type with macOS `leaks --atExit` on the test binary.
