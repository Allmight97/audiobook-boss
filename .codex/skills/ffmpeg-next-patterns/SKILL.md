---
name: ffmpeg-next-patterns
description: FFmpeg-next Rust bindings patterns for audiobook-boss. Use when writing audio encoding, decoding, resampling, metadata handling, or stream processing code. Covers encoder setup, frame pipeline, packet writing, and M4B container specifics.
---

# FFmpeg-Next Patterns for audiobook-boss

This skill captures project-specific patterns for the `ffmpeg-next` crate. Consult this before writing any audio processing code.

## Tool Cross-Check

This skill captures known patterns. If you need to verify, go deeper, or something seems
stale, use the `lib-research` skill (btca for source, Context7 for docs).

## Crate Import Convention

```rust
use ffmpeg_next as ff;
```

## Core Pipeline Architecture

```
Input → Decoder → Resampler (optional) → Accumulator → Encoder → Packet Writer → Output
```

| Stage | Location | Key Types |
|-------|----------|-----------|
| Decoder setup | `audio/processor/engine.rs` | `ff::codec::decoder::Audio` |
| Encoder setup | `audio/processor/encoder/context.rs` | `ff::codec::encoder::audio::Encoder` |
| Frame processing | `audio/processor/frame_pipeline.rs` | `ff::frame::Audio` |
| Packet writing | `audio/processor/encoder/write.rs` | `ff::Packet` |
| Metadata | `metadata/ffmpeg_bridge.rs` | `ff::Dictionary` |

## Encoder Setup Pattern

From `audio/processor/encoder/context.rs` (search for `open_as_with`):

```rust
// 1. Find codec
let codec = ff::encoder::find(ff::codec::Id::AAC)
    .ok_or_else(|| AppError::General("AAC encoder not found".into()))?;

// 2. Create context and configure
let mut ctx = ff::codec::context::Context::new()
    .encoder()
    .audio()
    .map_err(|e| AppError::General(format!("Open encoder failed: {e}")))?;

ctx.set_rate(sample_rate as i32);
ctx.set_channel_layout(ff::channel_layout::ChannelLayout::default(channels));
ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
ctx.set_time_base(ff::Rational(1, sample_rate as i32));

// 3. Set global header flag for M4B containers
if requires_global_header {
    ctx.set_flags(ff::codec::flag::Flags::GLOBAL_HEADER);
}

// 4. Open with options dictionary
let encoder = ctx.open_as_with(codec, options_dict)?;
```

**Critical**: Options must be passed via `open_as_with()`, not set on context directly.

## Frame → Encoder → Packet Pattern

From `audio/processor/encoder/write.rs` (search for `encode_and_write_frame`):

```rust
pub fn encode_and_write_frame(
    encoder: &mut ff::codec::encoder::audio::Encoder,
    frame: &ff::frame::Audio,
    output_context: &mut ff::format::context::Output,
    output_stream_index: usize,
    output_time_base: ff::Rational,
) -> Result<()> {
    // Send frame to encoder
    encoder.send_frame(frame)
        .map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?;

    // Receive all available packets
    let mut pkt = ff::Packet::empty();
    while encoder.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(output_stream_index);
        pkt.rescale_ts(encoder.time_base(), output_time_base);
        pkt.write_interleaved(output_context)?;
    }
    Ok(())
}
```

## Finalization Pattern

```rust
// Signal EOF to encoder
encoder.send_eof().ok();

// Drain remaining packets
let mut pkt = ff::Packet::empty();
while encoder.receive_packet(&mut pkt).is_ok() {
    pkt.set_stream(output_stream_index);
    pkt.rescale_ts(encoder.time_base(), output_time_base);
    pkt.write_interleaved(output_context)?;
}

// Write container trailer
output_context.write_trailer()?;
```

## Resampler Pattern

From `audio/processor/frame_pipeline.rs` (search for `resampler.run`):

```rust
let mut out = ff::frame::Audio::empty();
out.set_format(encoder.format());
out.set_channel_layout(encoder.channel_layout());
out.set_rate(encoder.rate());
out.set_samples(input_frame.samples());

unsafe {
    out.alloc(encoder.format(), input_frame.samples(), encoder.channel_layout());
}

resampler.run(&input_frame, &mut out)?;
```

## Metadata Dictionary Pattern

From `metadata/ffmpeg_bridge.rs` (search for `metadata_to_ffmpeg_dict`):

```rust
pub fn metadata_to_ffmpeg_dict(metadata: &AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    let mut dict = ff::Dictionary::new();

    if let Some(ref title) = metadata.title {
        dict.set("title", title);
    }
    if let Some(ref artist) = metadata.artist {
        dict.set("artist", artist);
        dict.set("album_artist", artist);  // Audiobook convention
    }
    // ... more fields

    dict.set("media_type", "2");  // iTunes audiobook type (stik=2)
    Ok(dict)
}

// Apply to output context
octx.set_metadata(dict);
```

## Cover Art Embedding (M4B)

From `metadata/ffmpeg_bridge.rs` (search for `add_cover_art_stream_pre_header`):

```rust
// 1. Add stream BEFORE write_header()
let cover_stream_info = add_cover_art_stream_pre_header(&mut octx, cover_data);

// 2. Set ATTACHED_PIC disposition via FFI (required for M4B)
unsafe {
    (*stream_ptr).disposition |= 0x0400;  // AV_DISPOSITION_ATTACHED_PIC
}

// 3. Write header
octx.write_header()?;

// 4. Write packet AFTER header
let mut pkt = ff::Packet::copy(cover_data);
pkt.set_stream(stream_index);
pkt.set_flags(ff::packet::flag::Flags::KEY);
pkt.set_pts(Some(0));
pkt.set_dts(Some(0));
pkt.write_interleaved(&mut octx)?;
```

## Input/Output Context Patterns

```rust
// Input
let mut ictx = ff::format::input(&path)?;
let stream = ictx.streams().best(ff::media::Type::Audio)?;
let decoder = ff::codec::context::Context::from_parameters(stream.parameters())?
    .decoder()
    .audio()?;

// Output
let mut octx = ff::format::output(&path)?;
let codec = ff::encoder::find(ff::codec::Id::AAC)?;
let mut ost = octx.add_stream(codec)?;
ost.set_time_base(encoder.time_base());
ost.set_parameters(&encoder);
octx.write_header()?;
// ... write packets ...
octx.write_trailer()?;
```

## Packet Iteration

```rust
for (stream, packet) in ictx.packets() {
    if stream.index() != audio_stream_index {
        continue;
    }
    decoder.send_packet(&packet)?;
    // ... receive frames ...
}
```

## Error Handling

Map ffmpeg errors to `AppError::Ffmpeg` or `AppError::General`:

```rust
// Direct mapping (from ffmpeg_next::Error)
#[error("FFmpeg error: {0}")]
Ffmpeg(#[from] ffmpeg_next::Error),

// Contextual wrapping
.map_err(|e| AppError::General(format!("Encoder send failed: {e}")))?
```

## Frame Contract Requirements

Before sending frame to encoder:
1. `frame.format()` == `encoder.format()`
2. `frame.channel_layout()` == `encoder.channel_layout()`
3. `frame.rate()` == `encoder.rate()`
4. `frame.samples()` > 0
5. `frame.pts()` must be set

## Sample Format by Encoder

| Encoder | Sample Format |
|---------|---------------|
| Native AAC | `F32(Planar)` |
| FDK HE-AAC | `I16(Packed)` |
| AAC-AT (Apple) | `I16(Packed)` |

## Common Gotchas

1. **Options via open_as_with**: Encoder options (bitrate, profile) must be passed as Dictionary to `open_as_with()`, not set on context.

2. **PTS before encoding**: Always set `frame.set_pts(Some(running_pts))` before sending to encoder.

3. **Time base rescaling**: Packets need `pkt.rescale_ts(encoder.time_base(), output.time_base())` before writing.

4. **Global header flag**: M4B/MP4 containers require `GLOBAL_HEADER` flag on encoder.

5. **Cover art timing**: Stream must be added pre-header, packet written post-header.

6. **EOF handling**: Always call `encoder.send_eof()` then drain remaining packets.

## References

- `docs/external-apis/ffmpeg-next.md`
- [context7: ffmpeg-next docs](/websites/rs_ffmpeg-next)
- [Codebase: encoder setup](src-tauri/src/audio/processor/encoder/context.rs)
- [Codebase: frame pipeline](src-tauri/src/audio/processor/frame_pipeline.rs)
- [Codebase: metadata bridge](src-tauri/src/metadata/ffmpeg_bridge.rs)
