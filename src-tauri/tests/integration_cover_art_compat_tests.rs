use audiobook_boss_lib::commands::metadata::save_metadata_to_file;
use audiobook_boss_lib::AudiobookMetadata;
use ffmpeg_next as ff;
use mp4ameta::Tag;
use std::path::Path;
use tempfile::TempDir;

// Minimal 1x1 JPEG (JFIF) header (valid tiny image) - using a common minimal pattern.
const MINIMAL_JPEG: &[u8] =
    b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";

fn write_minimal_m4b(output: &Path) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    octx.write_header().expect("write header");

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

#[test]
fn covr_only_is_visible_to_ffprobe() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("covr-only.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        },
    )
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert!(tag.artwork().is_some(), "covr atom should be present");

    let ictx = ff::format::input(&output).expect("open output");
    let has_attached_pic = ictx.streams().any(|stream| {
        stream
            .disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC)
    });
    assert!(
        has_attached_pic,
        "ffprobe should surface covr artwork as attached_pic for compatibility"
    );
}
