use super::*;
use abb_ffmpeg::{AudioShape, ChannelLayout, SampleFormat, Tags};

#[test]
fn upsample_resampler_output_capacity_prevents_bulk_eof_backlog() {
    let layout = ChannelLayout::default_for(1);
    let fmt = SampleFormat::F32_PLANAR;
    let shape = |rate| AudioShape {
        format: fmt,
        layout,
        rate,
    };
    let mut resampler = Resampler::new(shape(22_050), shape(44_100), &Tags::default())
        .expect("build 22.05k->44.1k resampler");

    let input_samples = 4096usize;
    let mut input = allocated_audio_frame(fmt, layout, 22_050, input_samples).expect("input frame");
    for v in input.plane_f32_mut(0).expect("input plane") {
        *v = 0.25;
    }

    let output_capacity = resampler_output_frame_samples(&resampler, input_samples);
    assert!(
        output_capacity >= input_samples * 2,
        "upsample output frame must be sized for the output-rate sample count"
    );
    let mut out =
        allocate_resampler_output_frame(&resampler, output_capacity).expect("output frame");
    resampler.convert(&input, &mut out).expect("resample run");

    let backlog = (input_samples * 2).saturating_sub(out.samples());
    assert!(
        backlog < RESAMPLER_FLUSH_CHUNK_SAMPLES,
        "run must leave only the filter delay for EOF, not bulk backlog (backlog={backlog})"
    );
}
