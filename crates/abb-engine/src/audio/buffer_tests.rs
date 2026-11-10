use super::{allocated_audio_frame, SampleAccumulator, SanitizeReport};
use abb_ffmpeg::{AudioFrame, ChannelLayout, SampleFormat};

const PLANAR: SampleFormat = SampleFormat::F32_PLANAR;
const PACKED: SampleFormat = SampleFormat::F32;

fn planar_frame(channels: u16, samples: &[f32]) -> AudioFrame {
    let layout = ChannelLayout::default_for(channels);
    let mut frame =
        allocated_audio_frame(PLANAR, layout, 44_100, samples.len()).expect("planar frame");
    for channel in 0..frame.planes() {
        frame
            .plane_f32_mut(channel)
            .expect("planar channel")
            .copy_from_slice(samples);
    }
    frame
}

fn packed_frame(channels: u16, interleaved: &[f32]) -> AudioFrame {
    let layout = ChannelLayout::default_for(channels);
    let mut frame = allocated_audio_frame(
        PACKED,
        layout,
        44_100,
        interleaved.len() / usize::from(channels),
    )
    .expect("packed frame");
    for (bytes, value) in frame
        .packed_bytes_mut()
        .expect("packed bytes")
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(interleaved)
    {
        bytes.copy_from_slice(&value.to_ne_bytes());
    }
    frame
}

fn accumulator(channels: usize, format: SampleFormat, frame_size: usize) -> SampleAccumulator {
    let layout = ChannelLayout::default_for(channels as u16);
    SampleAccumulator::new(channels, frame_size, 44_100, layout, format).expect("accumulator")
}

fn packed_values(frame: &AudioFrame, count: usize) -> Vec<f32> {
    frame.packed_bytes().expect("packed bytes")[..count * 4]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_ne_bytes(*bytes))
        .collect()
}

#[test]
fn missing_planar_channel_is_padded_with_silence() {
    let mut stereo = accumulator(2, PLANAR, 4);

    let ready = stereo
        .push_frame(&planar_frame(1, &[0.1, 0.2, 0.3, 0.4]), 4)
        .expect("push");

    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].plane_f32(0).expect("left"), &[0.1, 0.2, 0.3, 0.4]);
    assert_eq!(ready[0].plane_f32(1).expect("right"), &[0.0; 4]);
}

#[test]
fn planar_samples_clamp_finite_overshoot_and_zero_non_finite_values() {
    let mut mono = accumulator(1, PLANAR, 5);

    let ready = mono
        .push_frame(
            &planar_frame(1, &[1.5, -2.0, 0.5, f32::NAN, f32::NEG_INFINITY]),
            5,
        )
        .expect("push");

    assert_eq!(
        ready[0].plane_f32(0).expect("mono"),
        &[1.0, -1.0, 0.5, 0.0, 0.0]
    );
}

#[test]
fn packed_float_samples_clamp_finite_overshoot_and_zero_non_finite_values() {
    let mut mono = accumulator(1, PACKED, 5);

    let ready = mono
        .push_frame(
            &packed_frame(1, &[1.5, -2.0, 0.5, f32::INFINITY, f32::NAN]),
            5,
        )
        .expect("push");

    assert_eq!(packed_values(&ready[0], 5), [1.0, -1.0, 0.5, 0.0, 0.0]);
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

#[test]
fn only_the_requested_samples_are_buffered() {
    let mut planar = accumulator(2, PLANAR, 2);
    let mut stereo = planar_frame(2, &[0.1, 0.2, 0.3, 0.4, 0.5]);
    stereo
        .plane_f32_mut(1)
        .expect("right channel")
        .copy_from_slice(&[0.6, 0.7, 0.8, 0.9, 1.0]);
    let ready = planar.push_frame(&stereo, 3).expect("push");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].plane_f32(0).expect("left"), &[0.1, 0.2]);
    assert_eq!(ready[0].plane_f32(1).expect("right"), &[0.6, 0.7]);
    let tail = planar
        .flush_tail()
        .expect("flush")
        .expect("one buffered sample");
    assert_eq!(tail.plane_f32(0).expect("left"), &[0.3]);
    assert_eq!(tail.plane_f32(1).expect("right"), &[0.8]);

    let mut packed = accumulator(2, PACKED, 2);
    assert!(packed
        .push_frame(&packed_frame(2, &[0.1, 0.6, 0.2, 0.7, 0.3, 0.8]), 1)
        .expect("push")
        .is_empty());
    let tail = packed
        .flush_tail()
        .expect("flush")
        .expect("one buffered sample");
    assert_eq!(tail.samples(), 1);
    assert_eq!(packed_values(&tail, 2), [0.1, 0.6]);
}
