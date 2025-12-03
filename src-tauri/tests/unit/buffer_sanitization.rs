use audiobook_boss_lib::audio::buffer::SampleAccumulator;
use ffmpeg_next as ff;

#[test]
fn sanitize_clamps_and_fixes_non_finite() {
    let mut acc = SampleAccumulator::new(
        1,
        4,
        44_100,
        ff::channel_layout::ChannelLayout::MONO,
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
    );

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    frame.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    frame.set_rate(44_100);
    frame.set_samples(4);
    unsafe {
        frame.alloc(
            ff::format::Sample::F32(ff::format::sample::Type::Planar),
            4,
            ff::channel_layout::ChannelLayout::MONO,
        );
    }
    let plane = frame.data_mut(0);
    let samples: &mut [f32] = unsafe {
        std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, 4)
    };
    samples.copy_from_slice(&[f32::NAN, f32::INFINITY, -1.5, 0.5]);

    let mut produced = acc.push_frame(&frame);
    let out_frame = produced
        .pop()
        .or_else(|| acc.flush_tail(true))
        .expect("expected sanitized frame");

    let out_plane = out_frame.data(0);
    let out_samples: &[f32] = unsafe {
        std::slice::from_raw_parts(out_plane.as_ptr() as *const f32, out_frame.samples())
    };
    assert!(
        out_samples.iter().all(|v| v.is_finite()),
        "sanitizer should drop non-finite values"
    );
    assert_eq!(out_samples, &[0.0, 0.0, -1.0, 0.5]);
}

#[test]
fn s16_packed_copies_without_sanitization() {
    let mut acc = SampleAccumulator::new(
        2,
        2,
        44_100,
        ff::channel_layout::ChannelLayout::STEREO,
        ff::format::Sample::I16(ff::format::sample::Type::Packed),
    );

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(ff::format::Sample::I16(ff::format::sample::Type::Packed));
    frame.set_channel_layout(ff::channel_layout::ChannelLayout::STEREO);
    frame.set_rate(44_100);
    frame.set_samples(2);
    unsafe {
        frame.alloc(
            ff::format::Sample::I16(ff::format::sample::Type::Packed),
            2,
            ff::channel_layout::ChannelLayout::STEREO,
        );
    }
    let plane = frame.data_mut(0);
    let dst: &mut [i16] = unsafe {
        std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, 4)
    };
    dst.copy_from_slice(&[100, -200, 300, -400]);

    let mut produced = acc.push_frame(&frame);
    let out = produced
        .pop()
        .or_else(|| acc.flush_tail(true))
        .expect("expected flushed frame for s16 packed");

    let plane_out = out.data(0);
    let out_samples: &[i16] = unsafe {
        std::slice::from_raw_parts(plane_out.as_ptr() as *const i16, 4)
    };
    assert_eq!(out_samples, &[100, -200, 300, -400]);
}
