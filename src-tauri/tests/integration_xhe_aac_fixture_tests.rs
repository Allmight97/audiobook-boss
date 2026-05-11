use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::toolchain::ExternalToolchainPreference;
use audiobook_boss_lib::audio::{self, SampleRateConfig};
use audiobook_boss_lib::processing::{
    OutputConfig, PreviewConfig, ProcessingContext, ProcessingSession,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires ABB_XHE_AAC_FIXTURE and an external FFmpeg with libfdk_aac"]
async fn xhe_aac_fixture_encodes_short_external_fdk_preview_when_configured() {
    let input_path = fixture_path();
    assert!(
        input_path.exists() && input_path.is_file(),
        "ABB_XHE_AAC_FIXTURE must point at an xHE-AAC/USAC audiobook fixture"
    );

    let file_info = audio::get_file_list_info(std::slice::from_ref(&input_path))
        .expect("fixture should be inspectable");
    assert_eq!(file_info.valid_count, 1, "fixture should be valid");

    let settings = EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: EncoderChannelConfig::Auto,
        afterburner: true,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };
    let toolchain_preference = toolchain_preference();
    let adapter =
        audio::processor::resolve_processor_adapter(&settings, toolchain_preference.as_ref())
            .expect("external FDK adapter should resolve for fixture validation");
    adapter
        .validate_inputs(&file_info)
        .expect("selected decoder should be available in the external toolchain");

    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("xhe-aac-preview.m4b");
    let mut context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        settings,
        SampleRateConfig::Auto,
        OutputConfig::for_preview(&output_path),
    );
    context.preview = Some(PreviewConfig::new(20.0));

    let result = adapter
        .execute(
            context,
            file_info.files,
            file_info.selected_decoders,
            None,
            true,
        )
        .await
        .expect("external FDK preview should encode the xHE-AAC fixture");

    assert!(result.contains("Successfully created preview"));
    assert!(output_path.exists(), "preview output should exist");
    let output_info = audio::get_file_list_info(std::slice::from_ref(&output_path))
        .expect("preview output should be inspectable");
    assert_eq!(output_info.valid_count, 1, "preview output should be valid");
}

fn fixture_path() -> PathBuf {
    std::env::var_os("ABB_XHE_AAC_FIXTURE")
        .map(PathBuf::from)
        .expect("set ABB_XHE_AAC_FIXTURE to a local xHE-AAC/USAC audiobook fixture")
}

fn toolchain_preference() -> Option<ExternalToolchainPreference> {
    std::env::var("ABB_XHE_AAC_FFMPEG")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|override_path| ExternalToolchainPreference {
            override_path: Some(override_path),
        })
}
