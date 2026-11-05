use super::*;

#[test]
fn upsample_resampler_output_capacity_prevents_bulk_eof_backlog() {
    ff::init().expect("ffmpeg init");
    let layout = ff::ChannelLayout::default(1);
    let fmt = ff::format::Sample::F32(ff::format::sample::Type::Planar);
    let mut resampler =
        ff::software::resampling::Context::get(fmt, layout, 22_050, fmt, layout, 44_100)
            .expect("build 22.05k->44.1k resampler");

    let input_samples = 4096usize;
    let mut input = allocated_audio_frame(fmt, layout, 22_050, input_samples);
    for v in input.plane_mut::<f32>(0) {
        *v = 0.25;
    }

    let output_capacity = resampler_output_frame_samples(&resampler, input_samples);
    assert!(
        output_capacity >= input_samples * 2,
        "upsample output frame must be sized for the output-rate sample count"
    );
    let mut out = allocate_resampler_output_frame(&resampler, output_capacity);
    resampler.run(&input, &mut out).expect("resample run");

    let backlog = (input_samples * 2).saturating_sub(out.samples());
    assert!(
        backlog < RESAMPLER_FLUSH_CHUNK_SAMPLES,
        "run must leave only the filter delay for EOF, not bulk backlog (backlog={backlog})"
    );
}
