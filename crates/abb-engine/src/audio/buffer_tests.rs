use super::{SampleAccumulator, SanitizeReport};
use ffmpeg_next as ff;

const PLANAR: ff::format::Sample = ff::format::Sample::F32(ff::format::sample::Type::Planar);
const PACKED: ff::format::Sample = ff::format::Sample::F32(ff::format::sample::Type::Packed);

fn planar_frame(channels: i32, samples: &[f32]) -> ff::frame::Audio {
    let layout = ff::ChannelLayout::default(channels);
    let mut frame = ff::frame::Audio::empty();
    frame.set_format(PLANAR);
    frame.set_channel_layout(layout);
    frame.set_rate(44_100);
    frame.set_samples(samples.len());
    // SAFETY: Format, layout, rate, and sample count are set above, and `alloc` is called once on an empty
    // frame.
    unsafe {
        frame.alloc(PLANAR, samples.len(), layout);
    }
    for channel in 0..frame.planes() {
        frame.plane_mut::<f32>(channel).copy_from_slice(samples);
    }
    frame
}

fn packed_mono_frame(samples: &[f32]) -> ff::frame::Audio {
    let layout = ff::ChannelLayout::default(1);
    let mut frame = ff::frame::Audio::new(PACKED, samples.len(), layout);
    frame.set_rate(44_100);
    for (bytes, value) in frame.data_mut(0).chunks_exact_mut(4).zip(samples) {
        bytes.copy_from_slice(&value.to_ne_bytes());
    }
    frame
}

fn accumulator(
    channels: usize,
    format: ff::format::Sample,
    frame_size: usize,
) -> SampleAccumulator {
    ff::init().expect("ffmpeg init");
    let layout = ff::ChannelLayout::default(channels as i32);
    SampleAccumulator::new(channels, frame_size, 44_100, layout, format).expect("accumulator")
}

#[test]
fn missing_planar_channel_is_padded_with_silence() {
    let mut stereo = accumulator(2, PLANAR, 4);

    let ready = stereo.push_frame(&planar_frame(1, &[0.1, 0.2, 0.3, 0.4]));

    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].plane::<f32>(0), &[0.1, 0.2, 0.3, 0.4]);
    assert_eq!(ready[0].plane::<f32>(1), &[0.0; 4]);
}

#[test]
fn planar_samples_clamp_finite_overshoot_and_zero_non_finite_values() {
    let mut mono = accumulator(1, PLANAR, 5);

    let ready = mono.push_frame(&planar_frame(
        1,
        &[1.5, -2.0, 0.5, f32::NAN, f32::NEG_INFINITY],
    ));

    assert_eq!(ready[0].plane::<f32>(0), &[1.0, -1.0, 0.5, 0.0, 0.0]);
}

#[test]
fn packed_float_samples_clamp_finite_overshoot_and_zero_non_finite_values() {
    let mut mono = accumulator(1, PACKED, 5);

    let ready = mono.push_frame(&packed_mono_frame(&[
        1.5,
        -2.0,
        0.5,
        f32::INFINITY,
        f32::NAN,
    ]));

    let values: Vec<f32> = ready[0].data(0)[..20]
        .chunks_exact(4)
        .map(|bytes| f32::from_ne_bytes(bytes.try_into().expect("four bytes")))
        .collect();
    assert_eq!(values, [1.0, -1.0, 0.5, 0.0, 0.0]);
}

#[test]
fn sanitize_log_level_warns_only_for_non_finite_samples_or_large_clipping() {
    let level = |samples: &[f32]| {
        let mut report = SanitizeReport::default();
        for &sample in samples {
            report.sanitize(sample);
        }
        report.level()
    };

    assert_eq!(level(&[0.5, -0.9]), None);
    assert_eq!(level(&[1.05, -1.1]), Some(log::Level::Debug));
    assert_eq!(level(&[1.5]), Some(log::Level::Warn));
    assert_eq!(level(&[0.2, f32::NAN]), Some(log::Level::Warn));
}
