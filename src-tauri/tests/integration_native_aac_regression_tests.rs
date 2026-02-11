use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{self, OutputConfig, SampleRateConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

fn sample_mp3_path() -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir parent")
        .join("media")
        .join("media_20sec.mp3");
    if path.exists() && path.is_file() {
        Some(path)
    } else {
        None
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn native_aac_regression_encodes_valid_output() {
    let input_path = match sample_mp3_path() {
        Some(path) => path,
        None => {
            eprintln!("Skipping native AAC regression test - media fixture missing");
            return;
        }
    };

    let file_info = audio::get_file_list_info(std::slice::from_ref(&input_path))
        .expect("input media should be analyzable");
    assert_eq!(file_info.valid_count, 1, "fixture should be valid");
    let expected_duration = file_info.total_duration;

    let settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("native-regression.m4b");

    let session = Arc::new(audio::session::ProcessingSession::new());
    let context = audio::ProcessingContext::new_headless(
        session,
        settings,
        SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = audio::process_audiobook_with_context(context, file_info.files, None).await;
    assert!(
        result.is_ok(),
        "native AAC encode should complete without error: {:?}",
        result
    );
    assert!(output_path.exists(), "native AAC output file should exist");

    let output_info = audio::get_file_list_info(std::slice::from_ref(&output_path))
        .expect("output should be analyzable");
    assert_eq!(output_info.valid_count, 1, "output file should be valid");
    let actual_duration = output_info.files[0]
        .duration
        .expect("output should include duration");
    assert!(
        (actual_duration - expected_duration).abs() < 0.5,
        "output duration should be close to source duration (expected {expected_duration}, got {actual_duration})"
    );
}
