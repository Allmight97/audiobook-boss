use audiobook_boss_lib::audio::buffer::SampleAccumulator;
use audiobook_boss_lib::audio::processor::encoder::encode_and_write_frame;
use ffmpeg_next as ff;

#[test]
fn fastpath_enabled_processes_matching_frame() {
    let _ = ff::init();
    std::env::remove_var("ABB_DISABLE_FASTPATH");

    // Build a minimal encoder/output context
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(&"/tmp/test_fastpath.m4b").expect("create output");
    let mut ost = octx.add_stream(codec).expect("add stream");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("open encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::STEREO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let enc = enc_ctx.open_as(codec).expect("open encoder");

    ost.set_time_base(enc.time_base());
    ost.set_parameters(&enc);
    octx.write_header().expect("write header");

    // Build a frame that matches encoder contract
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
    let samples: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples())
    };
    samples.fill(0.1);

    // Accumulator should pass frame through unchanged size
    let mut acc = SampleAccumulator::new(
        enc.channel_layout().channels() as usize,
        enc.frame_size().max(1024) as usize,
        enc.rate() as u32,
        enc.channel_layout(),
        enc.format(),
    );

    for mut full in acc.push_frame(&frame) {
        full.set_pts(Some(0));
        let mut encoder = enc.clone();
        let mut octx_clone = octx.clone();
        encode_and_write_frame(
            &mut encoder,
            &full,
            &mut octx_clone,
            ost.index(),
            ost.time_base(),
        )
        .expect("encode frame");
    }
}
