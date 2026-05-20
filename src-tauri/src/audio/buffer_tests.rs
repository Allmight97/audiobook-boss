use super::SampleAccumulator;
use ffmpeg_next as ff;

const RATE: u32 = 22_050;
const FORMAT: ff::format::Sample = ff::format::Sample::F32(ff::format::sample::Type::Planar);
const LAYOUT: ff::channel_layout::ChannelLayout = ff::channel_layout::ChannelLayout::STEREO;

fn f32_planar_stereo_frame(left: &[f32], right: &[f32]) -> ff::frame::Audio {
    assert_eq!(left.len(), right.len());
    let _ = ff::init();

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(FORMAT);
    frame.set_channel_layout(LAYOUT);
    frame.set_rate(RATE);
    frame.set_samples(left.len());
    unsafe {
        frame.alloc(FORMAT, left.len(), LAYOUT);
    }

    frame.plane_mut::<f32>(0).copy_from_slice(left);
    frame.plane_mut::<f32>(1).copy_from_slice(right);
    frame
}

#[test]
fn f32_planar_stereo_push_preserves_both_channels() {
    let left = [0.10, 0.20, 0.30, 0.40];
    let right = [-0.10, -0.20, -0.30, -0.40];
    let frame = f32_planar_stereo_frame(&left, &right);
    let mut accumulator =
        SampleAccumulator::new(2, 4, RATE, LAYOUT, FORMAT).expect("create accumulator");

    let ready = accumulator.push_frame(&frame);

    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].plane::<f32>(0), left);
    assert_eq!(ready[0].plane::<f32>(1), right);
}

#[test]
fn f32_planar_stereo_flush_pads_each_channel_independently() {
    let left = [0.10, 0.20, 0.30];
    let right = [-0.10, -0.20, -0.30];
    let frame = f32_planar_stereo_frame(&left, &right);
    let mut accumulator =
        SampleAccumulator::new(2, 4, RATE, LAYOUT, FORMAT).expect("create accumulator");

    assert!(accumulator.push_frame(&frame).is_empty());
    let tail = accumulator.flush_tail(true).expect("flush padded tail");

    assert_eq!(tail.plane::<f32>(0), [0.10, 0.20, 0.30, 0.0]);
    assert_eq!(tail.plane::<f32>(1), [-0.10, -0.20, -0.30, 0.0]);
}

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

fn stereo_f32_frame(left: &[f32], right: &[f32]) -> ff::frame::Audio {
    assert_eq!(left.len(), right.len());
    let mut frame = ff::frame::Audio::new(
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
        left.len(),
        ff::channel_layout::ChannelLayout::STEREO,
    );
    frame.set_rate(44_100);
    frame.plane_mut::<f32>(0).copy_from_slice(left);
    frame.plane_mut::<f32>(1).copy_from_slice(right);
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
    )
    .expect("create mono f32 accumulator");

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
    )
    .expect("create stereo s16 accumulator");

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
    )
    .expect("create mono f32 accumulator");

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
fn f32_planar_flush_tail_preserves_remaining_samples_once() {
    let mut acc = SampleAccumulator::new(
        1,
        4,
        44_100,
        ff::channel_layout::ChannelLayout::MONO,
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
    )
    .expect("create mono f32 accumulator");

    let frame = mono_f32_frame(&[0.25, -0.25, 0.5]);
    assert!(acc.push_frame(&frame).is_empty());

    let tail = acc.flush_tail(true).expect("expected padded tail frame");
    let tail_plane = tail.data(0);
    let tail_samples: &[f32] =
        unsafe { std::slice::from_raw_parts(tail_plane.as_ptr() as *const f32, tail.samples()) };
    assert_eq!(tail.samples(), 4);
    assert_eq!(tail_samples, &[0.25, -0.25, 0.5, 0.0]);

    assert!(
        acc.flush_tail(true).is_none(),
        "tail flush should only emit the residual samples once"
    );
}

#[test]
fn f32_planar_stereo_preserves_channel_order_across_drain_cycles() {
    let mut acc = SampleAccumulator::new(
        2,
        4,
        44_100,
        ff::channel_layout::ChannelLayout::STEREO,
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
    )
    .expect("create stereo f32 accumulator");

    let expected_left: Vec<f32> = (0..11).map(|i| 0.05 * i as f32).collect();
    let expected_right: Vec<f32> = (0..11).map(|i| -0.05 * i as f32).collect();
    let mut observed_left = Vec::new();
    let mut observed_right = Vec::new();

    for (left, right) in expected_left.chunks(3).zip(expected_right.chunks(3)) {
        let frame = stereo_f32_frame(left, right);
        for full in acc.push_frame(&frame) {
            observed_left.extend_from_slice(full.plane::<f32>(0));
            observed_right.extend_from_slice(full.plane::<f32>(1));
        }
    }

    let tail = acc
        .flush_tail(false)
        .expect("expected unpadded stereo tail");
    observed_left.extend_from_slice(tail.plane::<f32>(0));
    observed_right.extend_from_slice(tail.plane::<f32>(1));

    assert_eq!(observed_left, expected_left);
    assert_eq!(observed_right, expected_right);
}

#[test]
fn s16_packed_preserves_interleaved_order_with_short_flush_tail() {
    let mut acc = SampleAccumulator::new(
        2,
        3,
        44_100,
        ff::channel_layout::ChannelLayout::STEREO,
        ff::format::Sample::I16(ff::format::sample::Type::Packed),
    )
    .expect("create stereo s16 accumulator");

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

#[test]
fn rejects_zero_frame_size() {
    match SampleAccumulator::new(
        1,
        0,
        44_100,
        ff::channel_layout::ChannelLayout::MONO,
        ff::format::Sample::F32(ff::format::sample::Type::Planar),
    ) {
        Ok(_) => panic!("zero frame size should be rejected"),
        Err(err) => assert!(
            err.to_string().contains("frame_size=0"),
            "error should identify the invalid frame size"
        ),
    }
}

#[test]
fn rejects_unsupported_sample_format() {
    match SampleAccumulator::new(
        1,
        1024,
        44_100,
        ff::channel_layout::ChannelLayout::MONO,
        ff::format::Sample::I32(ff::format::sample::Type::Packed),
    ) {
        Ok(_) => panic!("unsupported sample format should be rejected"),
        Err(err) => assert!(
            err.to_string()
                .contains("Unsupported encoder sample format"),
            "error should identify unsupported sample format"
        ),
    }
}
