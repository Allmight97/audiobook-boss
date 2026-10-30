use super::*;

/// With sample-rate conversion active, swresample withholds a filter tail
/// that `run()` alone never emits — the audio the pipeline used to drop at
/// every file boundary. The resampler drain must recover it so that
/// run + flush accounts for the full converted duration.
#[test]
fn rate_converting_resampler_tail_is_recovered_by_drain() {
    ff::init().expect("ffmpeg init");
    let layout = ff::ChannelLayout::default(1);
    let fmt = ff::format::Sample::F32(ff::format::sample::Type::Planar);
    let mut resampler =
        ff::software::resampling::Context::get(fmt, layout, 44_100, fmt, layout, 22_050)
            .expect("build 44.1k->22.05k resampler");

    let input_samples = 4096usize;
    let mut input = allocated_audio_frame(fmt, layout, 44_100, input_samples);
    for v in input.plane_mut::<f32>(0) {
        *v = 0.25;
    }

    let mut out = allocate_resampler_output_frame(&resampler, input_samples);
    resampler.run(&input, &mut out).expect("resample run");
    let converted = out.samples();

    let expected_total = input_samples / 2;
    assert!(
        converted < expected_total,
        "precondition: swr withholds a tail under rate conversion (run emitted {converted} of {expected_total})"
    );

    let tail = drain_resampler_sample_count(&mut resampler, 1024).expect("drain resampler");
    assert!(tail > 0, "drain must recover the held-back tail");

    let total = converted + tail;
    assert!(
        (total as i64 - expected_total as i64).abs() <= 32,
        "run + drain must account for the full converted duration (got {total}, want ~{expected_total})"
    );
}

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

    let expected_total = input_samples * 2;
    let tail = drain_resampler_sample_count(&mut resampler, RESAMPLER_FLUSH_CHUNK_SAMPLES)
        .expect("drain resampler");
    assert!(
        tail < RESAMPLER_FLUSH_CHUNK_SAMPLES,
        "EOF flush should only carry filter delay, not bulk upsample backlog (tail={tail})"
    );
    assert!(
        (out.samples() + tail).abs_diff(expected_total) <= 64,
        "run + drain must account for the upsampled duration"
    );
}
