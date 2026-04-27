use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{self, OutputConfig, SampleRateConfig};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tempfile::TempDir;
use tokio::sync::Mutex;

fn native_aac_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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

fn preview_output_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| std::borrow::Cow::from("output"));
    parent.join(format!("{}.preview.m4b", stem))
}

#[tokio::test(flavor = "multi_thread")]
async fn native_aac_regression_encodes_valid_output() {
    let _guard = native_aac_test_lock().lock().await;
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

    let result = audio::process_audiobook_with_context(context, file_info.files, None, true).await;
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

#[tokio::test(flavor = "multi_thread")]
async fn native_aac_preview_stops_near_requested_boundary() {
    let _guard = native_aac_test_lock().lock().await;
    let input_path = match sample_mp3_path() {
        Some(path) => path,
        None => {
            eprintln!("Skipping native AAC preview regression test - media fixture missing");
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
    let output_path = temp_dir.path().join("native-preview.m4b");
    let preview_path = preview_output_path(&output_path);
    let preview_seconds = 3.0;

    let session = Arc::new(audio::session::ProcessingSession::new());
    let mut context = audio::ProcessingContext::new_headless(
        session,
        settings,
        SampleRateConfig::Auto,
        OutputConfig::for_preview(&preview_path),
    );
    context.preview = Some(audio::context::PreviewConfig::new(preview_seconds));

    let result = audio::process_audiobook_with_context(context, file_info.files, None, true).await;
    assert!(
        result.is_ok(),
        "native AAC preview encode should complete without error: {:?}",
        result
    );
    assert!(preview_path.exists(), "preview output file should exist");

    let output_info = audio::get_file_list_info(std::slice::from_ref(&preview_path))
        .expect("preview output should be analyzable");
    assert_eq!(
        output_info.valid_count, 1,
        "preview output file should be valid"
    );
    let actual_duration = output_info.files[0]
        .duration
        .expect("preview output should include duration");
    assert!(
        actual_duration <= preview_seconds + 1.0,
        "preview output should stop near the requested boundary (expected <= {preview_seconds}, got {actual_duration})"
    );
    assert!(
        actual_duration < expected_duration,
        "preview output should not run to the end of the source (source duration {expected_duration}, preview duration {actual_duration})"
    );
}
