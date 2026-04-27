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

fn native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

async fn encode_with_fastpath_mode(
    input_path: &Path,
    output_path: &Path,
    disable_fastpath: bool,
) -> String {
    if disable_fastpath {
        std::env::set_var("ABB_DISABLE_FASTPATH", "1");
    } else {
        std::env::remove_var("ABB_DISABLE_FASTPATH");
    }

    let input_info =
        audio::get_file_list_info(&[input_path.to_path_buf()]).expect("input should be analyzable");
    assert_eq!(input_info.valid_count, 1, "input fixture should be valid");

    let context = audio::ProcessingContext::new_headless(
        Arc::new(audio::session::ProcessingSession::new()),
        native_encoder_settings(),
        SampleRateConfig::Auto,
        OutputConfig::new(output_path),
    );

    audio::process_audiobook_with_context(context, input_info.files, None, true)
        .await
        .expect("native AAC processing should complete")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "diagnostic native AAC parity check; run manually when touching fast-path/native encoder paths"]
async fn native_fastpath_parity_matches_core_output_properties() {
    let input_path = match sample_mp3_path() {
        Some(path) => path,
        None => {
            eprintln!("Skipping fast-path parity test - media fixture missing");
            return;
        }
    };

    let original_fastpath_var = std::env::var("ABB_DISABLE_FASTPATH").ok();
    let temp_dir = TempDir::new().expect("create temp dir");
    let fast_on_output = temp_dir.path().join("native-fastpath-on.m4b");
    let fast_off_output = temp_dir.path().join("native-fastpath-off.m4b");

    let fast_on_message = encode_with_fastpath_mode(&input_path, &fast_on_output, false).await;
    let fast_off_message = encode_with_fastpath_mode(&input_path, &fast_off_output, true).await;

    match original_fastpath_var {
        Some(value) => std::env::set_var("ABB_DISABLE_FASTPATH", value),
        None => std::env::remove_var("ABB_DISABLE_FASTPATH"),
    }

    assert!(
        fast_on_message.contains("Successfully created audiobook"),
        "Expected success message for fast-path ON: {}",
        fast_on_message
    );
    assert!(
        fast_off_message.contains("Successfully created audiobook"),
        "Expected success message for fast-path OFF: {}",
        fast_off_message
    );

    let fast_on_info = audio::get_file_list_info(std::slice::from_ref(&fast_on_output))
        .expect("fast-path ON output should be analyzable");
    let fast_off_info = audio::get_file_list_info(std::slice::from_ref(&fast_off_output))
        .expect("fast-path OFF output should be analyzable");

    let on_file = &fast_on_info.files[0];
    let off_file = &fast_off_info.files[0];
    let on_duration = on_file.duration.expect("fast-path ON duration");
    let off_duration = off_file.duration.expect("fast-path OFF duration");

    assert!(
        (on_duration - off_duration).abs() < 0.05,
        "Durations should remain near-identical (on={on_duration}, off={off_duration})"
    );
    assert_eq!(
        on_file.sample_rate, off_file.sample_rate,
        "Sample rates should match between fast-path modes"
    );
    assert_eq!(
        on_file.channels, off_file.channels,
        "Channel count should match between fast-path modes"
    );

    let on_bitrate = on_file.bitrate.expect("fast-path ON bitrate");
    let off_bitrate = off_file.bitrate.expect("fast-path OFF bitrate");
    assert!(
        (on_bitrate as i32 - off_bitrate as i32).abs() <= 1_500,
        "Bitrate delta should remain within 1500 bps (on={on_bitrate}, off={off_bitrate})"
    );
}
