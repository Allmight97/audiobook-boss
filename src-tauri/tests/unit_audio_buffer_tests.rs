use audiobook_boss_lib::audio::buffer::SampleAccumulator;
use ffmpeg_next as ff;

fn mono_f32_frame(samples: &[f32]) -> ff::frame::Audio {
    let mut frame = ff::frame::Audio::empty();
    frame.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    frame.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    frame.set_rate(44_100);
    frame.set_samples(samples.len());
    unsafe {
        frame.alloc(
            ff::format::Sample::F32(ff::format::sample::Type::Planar),
            samples.len(),
            ff::channel_layout::ChannelLayout::MONO,
        );
    }
    let plane = frame.data_mut(0);
    let dst: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, samples.len()) };
    dst.copy_from_slice(samples);
    frame
}

fn stereo_s16_frame(samples_per_channel: usize, interleaved: &[i16]) -> ff::frame::Audio {
    let mut frame = ff::frame::Audio::empty();
    frame.set_format(ff::format::Sample::I16(ff::format::sample::Type::Packed));
    frame.set_channel_layout(ff::channel_layout::ChannelLayout::STEREO);
    frame.set_rate(44_100);
    frame.set_samples(samples_per_channel);
    unsafe {
        frame.alloc(
            ff::format::Sample::I16(ff::format::sample::Type::Packed),
            samples_per_channel,
            ff::channel_layout::ChannelLayout::STEREO,
        );
    }
    let plane = frame.data_mut(0);
    let dst: &mut [i16] = unsafe {
        std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, interleaved.len())
    };
    dst.copy_from_slice(interleaved);
    frame
}

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
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, 4) };
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
    let dst: &mut [i16] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, 4) };
    dst.copy_from_slice(&[100, -200, 300, -400]);

    let mut produced = acc.push_frame(&frame);
    let out = produced
        .pop()
        .or_else(|| acc.flush_tail(true))
        .expect("expected flushed frame for s16 packed");

    let plane_out = out.data(0);
    let out_samples: &[i16] =
        unsafe { std::slice::from_raw_parts(plane_out.as_ptr() as *const i16, 4) };
    assert_eq!(out_samples, &[100, -200, 300, -400]);
}

#[test]
fn f32_planar_preserves_order_across_multiple_consumption_cycles() {
    let mut acc = SampleAccumulator::new(
        1,
        4,
        44_100,
        ff::channel_layout::ChannelLayout::MONO,
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
    );

    let expected: Vec<f32> = (0..15).map(|i| i as f32 / 20.0).collect();
    let mut observed = Vec::new();

    for chunk in expected.chunks(3) {
        let frame = mono_f32_frame(chunk);
        for full in acc.push_frame(&frame) {
            let plane = full.data(0);
            let out: &[f32] =
                unsafe { std::slice::from_raw_parts(plane.as_ptr() as *const f32, full.samples()) };
            observed.extend_from_slice(out);
        }
    }

    let tail = acc.flush_tail(false).expect("expected short tail frame");
    let tail_plane = tail.data(0);
    let tail_samples: &[f32] =
        unsafe { std::slice::from_raw_parts(tail_plane.as_ptr() as *const f32, tail.samples()) };
    observed.extend_from_slice(tail_samples);

    assert_eq!(observed, expected);
}

#[test]
fn s16_packed_preserves_interleaved_order_with_short_flush_tail() {
    let mut acc = SampleAccumulator::new(
        2,
        3,
        44_100,
        ff::channel_layout::ChannelLayout::STEREO,
        ff::format::Sample::I16(ff::format::sample::Type::Packed),
    );

    let frame_a = stereo_s16_frame(2, &[1, -1, 2, -2]);
    let frame_b = stereo_s16_frame(2, &[3, -3, 4, -4]);

    let mut produced = acc.push_frame(&frame_a);
    assert!(
        produced.is_empty(),
        "first partial chunk should not produce a full frame"
    );
    produced.extend(acc.push_frame(&frame_b));

    let full = produced.pop().expect("expected one full frame");
    let full_plane = full.data(0);
    let full_samples: &[i16] =
        unsafe { std::slice::from_raw_parts(full_plane.as_ptr() as *const i16, 6) };
    assert_eq!(full_samples, &[1, -1, 2, -2, 3, -3]);

    let tail = acc.flush_tail(false).expect("expected unpadded short tail");
    let tail_plane = tail.data(0);
    let tail_samples: &[i16] =
        unsafe { std::slice::from_raw_parts(tail_plane.as_ptr() as *const i16, 2) };
    assert_eq!(tail_samples, &[4, -4]);
}
