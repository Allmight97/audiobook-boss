use std::path::Path;

use crate::audio::{
    supported_audio_import_metadata, validate_encoder_settings, validate_input_audio_path,
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};

#[test]
fn audio_contract_supported_import_metadata_exposes_m4b_family() {
    let metadata = supported_audio_import_metadata();
    assert!(
        metadata
            .formats
            .iter()
            .any(|format| format.extension == "m4b"),
        "supported import metadata should include m4b"
    );
}

#[test]
fn audio_contract_rejects_missing_input_path() {
    let err = validate_input_audio_path(Path::new(
        "/definitely/not/a/real/audiobook-boss-input.m4b",
    ))
        .expect_err("missing path should fail validation");
    let message = err.to_string().to_lowercase();
    assert!(
        message.contains("not found")
            || message.contains("no such file")
            || message.contains("exist")
            || message.contains("cannot read"),
        "expected filesystem validation failure, got: {err}"
    );
}

#[test]
fn audio_contract_rejects_invalid_encoder_bitrate() {
    let settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 1,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        afterburner: true,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    let err = validate_encoder_settings(&settings).expect_err("invalid bitrate should fail");
    assert!(
        !err.to_string().is_empty(),
        "encoder validation should return a concrete error"
    );
}
