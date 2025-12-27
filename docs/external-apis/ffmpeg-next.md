## FFmpeg-next integration guide (audio, PTS/time_base, progress)

This guide captures the integration patterns we use with `ffmpeg-next` for audiobook processing. It complements official references and anchors the exact contracts we rely on in this codebase.

### Where used
- `src-tauri/src/audio/processor/encoder/` (encoder selection and options)
- `src-tauri/src/audio/processor/frame_pipeline.rs` (frame sizing, PTS handling)
- `src-tauri/src/audio/processor/streams.rs` (stream setup)
- `src-tauri/src/audio/buffer.rs` (sanitization and frame construction)
- `src-tauri/src/metadata/ffmpeg_bridge.rs` (non-MP4 metadata + remux; MP4/M4B uses mp4ameta)

Metadata boundary: MP4/M4B metadata reads/writes are handled by `mp4ameta` (`metadata/mp4ameta_bridge.rs`), with ffmpeg-next handling audio processing and non-MP4 metadata workflows. Cover art embedded during muxing still uses ffmpeg-next attached-pic streams.

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

- We add an attached-pic stream pre-header, then write a post-header image packet with KEY flag and PTS/DTS=0. There is no Lofty fallback; ffmpeg-next is the single writer.

### References

- FFmpeg-next (Rust): [docs.rs – ffmpeg-next](https://docs.rs/ffmpeg-next/latest/ffmpeg_next/)
- FFmpeg (timestamps/time base): [FFmpeg docs – muxing/demuxing and timestamps](https://ffmpeg.org/)
- FFmpeg AAC encoder options (aac_coder): [FFmpeg docs – aac encoder options](https://ffmpeg.org/ffmpeg-codecs.html#aac-1)

### Apple AAC (aac_at) selection

Status: **Implemented**. The current implementation prefers FFmpeg’s `aac_at` encoder on macOS when it is present in the local FFmpeg build. We detect availability by querying the encoder registry (ffmpeg-next → `avcodec_find_encoder_by_name("aac_at")`) and gate by OS.

The resolver (`settings_encoder.rs`) chooses the best available encoder in the order: FDK > Apple (aac_at) > Native AAC. Hardware capability probing via AudioToolbox is still planned for future refinement.

Code touchpoints:
- `src-tauri/src/audio/settings_encoder.rs`
- `src-tauri/src/audio/processor/encoder/`
- `src/types/audio.ts`, `src/types/encoder.ts`

Constraints:
- ffmpeg-next only (no shell FFmpeg)
- macOS (Apple Silicon) is the target platform



### Frame contract & guards (recap)

- Encoder frame validity
  - Sample format: planar F32; channels/layout must match encoder configuration.
  - `nb_samples > 0`; if encoder reports `frame_size == 0` (variable), accumulate to a canonical size (1024) for consistency before sending.
  - All planes must be allocated with sufficient capacity before use (resampler writes into pre-allocated frames; it will not auto-alloc).
  - Samples must be finite and clamped to [-1.0, 1.0] before encoding (sanitization centralized in `audio/buffer.rs`).

- PTS/time base
  - Encoder time base is 1 / sample_rate; set `frame.pts = running_pts` and advance `running_pts += frame.samples()` for each encoded frame.
  - Rescale packet timestamps from encoder time base to output stream time base before writing.

- Fast-path safety
  - Only skip resampling if decoded frames already match encoder format, layout, and sample rate; otherwise resample first.

- Debug-only validator (development builds)
  - Assert planes allocated and perform a light finite-sample spot check prior to `avcodec_send_frame`.
  - Log a single fallback notice when using canonical accumulation for `frame_size == 0`.
