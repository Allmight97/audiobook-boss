use std::path::Path;

use crate::audio::SampleRateConfig;

use super::media_fixtures::MediaLane;
use abb_test_fixtures::media::{audio_duration_ts, decode_pcm_f32, ffmpeg};

fn exact_wav(path: &Path, rate: u32, samples: u64) {
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &format!("sine=frequency=440:sample_rate={rate}"),
        "-af",
        &format!("atrim=end_sample={samples}"),
        "-c:a",
        "pcm_s16le",
        "-y",
        path.to_str().expect("utf8"),
    ]);
}

#[tokio::test]
async fn a_native_aac_upsample_keeps_every_sample_of_the_book() {
    let dir = tempfile::TempDir::new().expect("dir");
    for (sources, rate, expected) in [
        (vec![22_117_u64], 44_100_u32, 44_234_u64),
        (vec![22_117, 11_003], 44_100, 66_240),
        (vec![22_117], 48_000, 48_146),
    ] {
        let inputs: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let p = dir.path().join(format!("{rate}-{}-{i}.wav", sources.len()));
                exact_wav(&p, 22_050, *n);
                p
            })
            .collect();
        let lane = MediaLane::for_inputs(inputs).with_sample_rate(SampleRateConfig::Explicit(rate));
        let output = lane.process(None).await;
        assert_eq!(audio_duration_ts(&output), expected, "declared");
        assert_eq!(decode_pcm_f32(&output).len() as u64, expected, "decoded");
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}
