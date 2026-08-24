use super::*;
use ffmpeg_next as ff;

const SAMPLE_RATE: u32 = 44_100;
const FRAME_SIZE: usize = 8;

fn init_ffmpeg() {
    ff::init().expect("ffmpeg init");
}

fn f32_planar() -> ff::format::Sample {
    ff::format::Sample::F32(ff::format::sample::Type::Planar)
}

fn i16_packed() -> ff::format::Sample {
    ff::format::Sample::I16(ff::format::sample::Type::Packed)
}

fn layout(channels: i32) -> ff::channel_layout::ChannelLayout {
    ff::channel_layout::ChannelLayout::default(channels)
}

fn new_accumulator(
    channels: usize,
    frame_size: usize,
    format: ff::format::Sample,
) -> SampleAccumulator {
    SampleAccumulator::new(
        channels,
        frame_size,
        SAMPLE_RATE,
        layout(channels as i32),
        format,
    )
    .expect("supported SampleAccumulator config")
}

fn alloc_audio_frame(
    format: ff::format::Sample,
    channels: i32,
    samples: usize,
) -> ff::frame::Audio {
    let channel_layout = layout(channels);
    let mut frame = ff::frame::Audio::empty();
    frame.set_format(format);
    frame.set_channel_layout(channel_layout);
    frame.set_rate(SAMPLE_RATE);
    frame.set_samples(samples);
    unsafe {
        frame.alloc(format, samples, channel_layout);
    }
    frame
}

fn fill_f32_plane(frame: &mut ff::frame::Audio, channel: usize, values: &[f32]) {
    let plane = frame.plane_mut::<f32>(channel);
    assert!(
        plane.len() >= values.len(),
        "typed plane {channel} is shorter than the values being written"
    );
    plane[..values.len()].copy_from_slice(values);
}

fn f32_plane_samples(frame: &ff::frame::Audio, channel: usize) -> Vec<f32> {
    let take = frame.samples();
    let plane = frame.plane::<f32>(channel);
    assert!(
        plane.len() >= take,
        "typed plane {channel} is shorter than the reported sample count"
    );
    plane[..take].to_vec()
}

fn fill_i16_packed(frame: &mut ff::frame::Audio, interleaved: &[i16]) {
    let plane = frame.data_mut(0);
    let needed_bytes = std::mem::size_of_val(interleaved);
    assert!(
        plane.len() >= needed_bytes,
        "packed I16 frame data(0) is shorter than the interleaved payload"
    );
    // SAFETY: `data_mut(0)` is the allocated packed S16 plane; we only view the
    // native-endian sample payload (`interleaved.len()` values), not linesize padding.
    let dst = unsafe {
        std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut i16, interleaved.len())
    };
    dst.copy_from_slice(interleaved);
}

fn i16_packed_samples(frame: &ff::frame::Audio, channels: usize) -> Vec<i16> {
    let take_total = frame.samples() * channels;
    let plane = frame.data(0);
    let needed_bytes = take_total * std::mem::size_of::<i16>();
    assert!(
        plane.len() >= needed_bytes,
        "packed I16 drain must expose samples through data(0)"
    );
    // SAFETY: production `drain_one_s16_packed` writes native-endian i16 into
    // `data_mut(0)`; this read uses the same plane and sample count.
    let src = unsafe { std::slice::from_raw_parts(plane.as_ptr() as *const i16, take_total) };
    src.to_vec()
}

/// Catches a silent/dropped right plane if `push_frame` only copies channel 0
/// and `drain_one` still reports a stereo frame.
#[test]
fn f32_planar_stereo_push_then_drain_keeps_both_planes_and_matching_sample_counts() {
    init_ffmpeg();
    let left = [0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80];
    let right = [-0.15, -0.25, -0.35, -0.45, -0.55, -0.65, -0.75, -0.85];

    let mut input = alloc_audio_frame(f32_planar(), 2, FRAME_SIZE);
    fill_f32_plane(&mut input, 0, &left);
    fill_f32_plane(&mut input, 1, &right);

    let mut accumulator = new_accumulator(2, FRAME_SIZE, f32_planar());
    let drained = accumulator.push_frame(&input);
    assert_eq!(
        drained.len(),
        1,
        "one exact encoder frame should drain from a full stereo push"
    );

    let out = &drained[0];
    assert_eq!(out.samples(), FRAME_SIZE);
    assert_eq!(out.planes(), 2, "drained frame must keep a right plane");
    assert_eq!(
        f32_plane_samples(out, 0),
        left,
        "left plane must survive push_frame -> drain_one"
    );
    assert_eq!(
        f32_plane_samples(out, 1),
        right,
        "right plane must survive push_frame -> drain_one (not silence)"
    );
}

/// Policy: silence-pad channel `ch > 0` only when typed `frame.planes()` omits
/// that channel (`ch >= planes`). Do not treat `data(ch)` / byte linesize as
/// missing-plane truth, and do not copy or drop the present plane.
#[test]
fn missing_plane_ch_gt_0_pads_silence_only_when_frame_planes_omits_that_channel() {
    init_ffmpeg();
    let left = [0.11, 0.22, 0.33, 0.44, 0.55, 0.66, 0.77, 0.88];

    let mut mono = alloc_audio_frame(f32_planar(), 1, FRAME_SIZE);
    assert_eq!(
        mono.planes(),
        1,
        "precondition: mono planar input reports one typed plane"
    );
    fill_f32_plane(&mut mono, 0, &left);

    let mut accumulator = new_accumulator(2, FRAME_SIZE, f32_planar());
    let drained = accumulator.push_frame(&mono);
    assert_eq!(drained.len(), 1);

    let out = &drained[0];
    assert_eq!(out.samples(), FRAME_SIZE);
    assert_eq!(
        f32_plane_samples(out, 0),
        left,
        "present plane 0 must be copied, not replaced with silence"
    );
    assert_eq!(
        f32_plane_samples(out, 1),
        [0.0; FRAME_SIZE],
        "channel 1 is absent from frame.planes() so only that plane is silence-padded"
    );
}

/// Catches flush(true) inventing extra full frames of silence, or flush(false)
/// padding a leftover tail up to `frame_size`.
#[test]
fn flush_tail_true_pads_exactly_one_short_frame_false_does_not_invent_a_full_frame() {
    init_ffmpeg();
    let tail = [0.31, -0.42, 0.53];
    assert!(tail.len() < FRAME_SIZE);

    let mut short = alloc_audio_frame(f32_planar(), 2, tail.len());
    fill_f32_plane(&mut short, 0, &tail);
    fill_f32_plane(&mut short, 1, &tail);

    let mut pad_on = new_accumulator(2, FRAME_SIZE, f32_planar());
    assert!(
        pad_on.push_frame(&short).is_empty(),
        "a short push must not drain a full frame"
    );
    let padded = pad_on
        .flush_tail(true)
        .expect("flush_tail(true) must emit the padded leftover");
    assert_eq!(
        padded.samples(),
        FRAME_SIZE,
        "flush_tail(true) pads exactly one short leftover to one encoder frame"
    );
    let mut expected = tail.to_vec();
    expected.extend(std::iter::repeat_n(0.0f32, FRAME_SIZE - tail.len()));
    assert_eq!(f32_plane_samples(&padded, 0), expected);
    assert_eq!(f32_plane_samples(&padded, 1), expected);
    assert!(
        pad_on.flush_tail(true).is_none(),
        "flush_tail(true) must not invent a second full frame after the leftover"
    );

    let mut pad_off = new_accumulator(2, FRAME_SIZE, f32_planar());
    assert!(pad_off.push_frame(&short).is_empty());
    let unpadded = pad_off
        .flush_tail(false)
        .expect("flush_tail(false) still drains the leftover samples");
    assert_eq!(
        unpadded.samples(),
        tail.len(),
        "flush_tail(false) must not invent a full encoder frame"
    );
    assert_eq!(f32_plane_samples(&unpadded, 0), tail);
    assert_eq!(f32_plane_samples(&unpadded, 1), tail);
}

/// Catches L/R swap or planar indexing on the Apple AAC packed-I16 `data_mut(0)`
/// path. Host-testable without AudioToolbox encode.
#[test]
fn i16_packed_drain_preserves_interleaved_lr_order_through_data_mut_0() {
    init_ffmpeg();
    // Distinct L/R magnitudes so a channel swap is obvious: L negative, R positive.
    let interleaved = [-1000i16, 2000, -3000, 4000, -5000, 6000, -7000, 8000];
    let frame_size = interleaved.len() / 2;

    let mut input = alloc_audio_frame(i16_packed(), 2, frame_size);
    fill_i16_packed(&mut input, &interleaved);

    let mut accumulator = new_accumulator(2, frame_size, i16_packed());
    let drained = accumulator.push_frame(&input);
    assert_eq!(drained.len(), 1);

    let out = &drained[0];
    assert_eq!(out.samples(), frame_size);
    assert_eq!(
        i16_packed_samples(out, 2),
        interleaved,
        "packed drain must keep interleaved L/R order through data(0)"
    );
}

/// Catches constructing a storage/format mismatch (or a zero-sized frame plan)
/// that would later truncate or silently corrupt encoder bytes.
#[test]
fn construct_rejects_zero_frame_size_and_unsupported_formats_as_anti_corruption_guard() {
    init_ffmpeg();
    let stereo = layout(2);

    let Err(zero) = SampleAccumulator::new(2, 0, SAMPLE_RATE, stereo, f32_planar()) else {
        panic!("frame_size == 0 must fail closed");
    };
    assert!(
        zero.to_string().contains("frame_size=0"),
        "zero frame_size error should name the anti-corruption guard, got {zero}"
    );

    let unsupported = [
        ff::format::Sample::F32(ff::format::sample::Type::Packed),
        ff::format::Sample::I16(ff::format::sample::Type::Planar),
        ff::format::Sample::U8(ff::format::sample::Type::Packed),
    ];
    for format in unsupported {
        let Err(err) = SampleAccumulator::new(2, FRAME_SIZE, SAMPLE_RATE, stereo, format) else {
            panic!("unsupported format {format:?} must fail closed");
        };
        let message = err.to_string();
        assert!(
            message.contains("Unsupported encoder sample format"),
            "unsupported format {format:?} should fail as the anti-corruption guard, got {message}"
        );
    }
}
