## FFmpeg-next integration guide (audio, PTS/time_base, progress)

This guide captures the integration patterns we use with `ffmpeg-next` for audiobook processing. It complements official references and anchors the exact contracts we rely on in this codebase.

### Core audio timestamp contract

- Set encoder time base to 1 / sample_rate.
- Maintain a monotonically increasing running PTS counter in encoder time base units.
- For every ready-to-encode frame, set `frame.set_pts(Some(running_pts))` and then increment by `frame.samples()`.
- After encoding, rescale packet timestamps from encoder time base to the output stream time base before writing.

Rust patterns used here:

```rust
// Set PTS in encoder time base units and advance by frame.samples()
full.set_pts(Some(*ctx.running_pts));
*ctx.running_pts += full.samples() as i64;
```

```rust
// Rescale packet timestamps from encoder time base → output stream time base
pkt.rescale_ts(encoder.time_base(), output_time_base);
pkt.write_interleaved(output_context)?;
```

Why this works well for audio:

- With time base = 1/rate, PTS becomes “number of output samples so far”, which is precise for PCM-to-AAC pipelines.
- Progress can be computed as seconds = PTS / rate, then mapped to UI ranges.

Example progress mapping used in this repo (throttled at 200ms):

```rust
let current_seconds = *ctx.running_pts as f64 / ctx.target_sample_rate as f64;
let percentage = converting_percentage_from_seconds(current_seconds, ctx.total_duration);
emitter.emit_converting_progress(percentage, "Converting and merging audio files...", ...);
```

### Encoder setup essentials

- Sample format: `ff::format::Sample::F32(ff::format::sample::Type::Planar)`
- Channel layout: `ff::channel_layout::ChannelLayout::default(target_channels)`
- Time base: `ff::Rational(1, sample_rate as i32)`
- Bitrate: set from UI (e.g., 64 kbps for audiobooks)

Optional tunings we support:

- `strict=experimental` (where accepted)
- `aac_coder=twoloop` for improved low-bitrate quality

Both options are applied via `av_opt_set` on the underlying encoder context, with robust error logging and safe fallbacks if unsupported.

### Decoder + resampler fast-path

- Fast-path: If a decoded frame already matches encoder format, layout, and rate, we skip resampling and send the frame through the accumulator directly.
- Otherwise, we resample into an output frame matching the encoder contract, then accumulate and encode.

Safety: Accumulator constructs exactly-sized frames for the encoder and sanitizes samples (finite; clamped to [-1, 1]). A debug-only validator enforces the frame contract prior to encoding.

### Packet writing & trailer

- Receive packets in a loop; set the output stream index, rescale PTS/DTS, and write interleaved to the output context.
- After `send_eof`, drain remaining packets and `write_trailer()`.

### Cover art (native first)

- We attempt to add an attached-pic stream pre-header, then write a post-header image packet with KEY flag and PTS/DTS=0. If this fails or is not detected later, we use Lofty fallback in finalize stage.

### References

- FFmpeg-next (Rust): [docs.rs – ffmpeg-next](https://docs.rs/ffmpeg-next/latest/ffmpeg_next/)
- FFmpeg (timestamps/time base): [FFmpeg docs – muxing/demuxing and timestamps](https://ffmpeg.org/)
- FFmpeg AAC encoder options (aac_coder): [FFmpeg docs – aac encoder options](https://ffmpeg.org/ffmpeg-codecs.html#aac-1)


