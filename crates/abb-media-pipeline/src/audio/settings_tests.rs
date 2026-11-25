use super::*;
use crate::audio::{validate_encoder_settings, BitrateMode, ChannelConfig, EncoderSettings};
use crate::errors::{AppErrorCode, AppErrorEnvelope};
use std::path::PathBuf;
use tempfile::TempDir;

fn code_of(error: &AppError) -> AppErrorCode {
    AppErrorEnvelope::from(error).code
}

#[test]
fn automatic_rate_changes_only_to_satisfy_the_encoder_profile() {
    for (encoder, profile, source, expected) in [
        (EncoderType::Faac, FaacProfile::HeAacV1, 22_050, 32_000),
        (EncoderType::Faac, FaacProfile::HeAacV1, 96_000, 48_000),
        (EncoderType::Faac, FaacProfile::AacLc, 22_050, 22_050),
        (EncoderType::NativeAac, FaacProfile::Auto, 44_100, 44_100),
        (EncoderType::Opus, FaacProfile::Auto, 22_050, 24_000),
        (EncoderType::Opus, FaacProfile::Auto, 44_100, 48_000),
    ] {
        assert_eq!(automatic_sample_rate(encoder, profile, source), expected);
    }
}

#[test]
fn faac_rejects_an_explicit_rate_outside_its_capability() {
    let error = validate_encoder_sample_rate(
        EncoderType::Faac,
        FaacProfile::HeAacV1,
        &SampleRateConfig::Explicit(22050),
    )
    .expect_err("FAAC should reject rates below 32 kHz");
    assert!(error.to_string().contains("does not support 22050 Hz"));
}

fn base_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Auto,
        native_aac_speed: 0,
        faac_profile: crate::audio::FaacProfile::Auto,
    }
}

#[test]
fn target_bitrate_uses_each_encoders_bounds() {
    for (encoder_type, bitrate_mode, kbps, valid) in [
        (EncoderType::NativeAac, BitrateMode::Cbr, 0, false),
        (EncoderType::NativeAac, BitrateMode::Cbr, 1, true),
        (EncoderType::NativeAac, BitrateMode::Cbr, 1152, true),
        (EncoderType::NativeAac, BitrateMode::Cbr, 1153, false),
        (EncoderType::Opus, BitrateMode::VbrTarget, 5, false),
        (EncoderType::Opus, BitrateMode::VbrTarget, 6, true),
        (EncoderType::Opus, BitrateMode::VbrTarget, 510, true),
        (EncoderType::Opus, BitrateMode::VbrTarget, 511, false),
    ] {
        let settings = EncoderSettings {
            encoder_type,
            bitrate_mode,
            bitrate_kbps: kbps,
            ..base_settings()
        };
        assert_eq!(
            validate_encoder_settings(&settings).is_ok(),
            valid,
            "{encoder_type:?} {kbps} kbps"
        );
    }
}

#[test]
fn native_speed_accepts_boundaries_and_rejects_out_of_range_intent() {
    let mut settings = base_settings();
    for encoder_type in [EncoderType::NativeAac, EncoderType::Auto] {
        settings.encoder_type = encoder_type;
        for speed in [0, 4] {
            settings.native_aac_speed = speed;
            assert!(validate_encoder_settings(&settings).is_ok());
        }
        settings.native_aac_speed = 5;
        assert!(validate_encoder_settings(&settings).is_err());
    }
}

#[test]
fn inactive_encoder_controls_do_not_constrain_selected_mode() {
    let mut settings = base_settings();
    settings.encoder_type = EncoderType::Faac;
    settings.bitrate_mode = BitrateMode::Vbr(100);
    settings.bitrate_kbps = 0;
    settings.native_aac_speed = 255;
    assert!(validate_encoder_settings(&settings).is_ok());
}

#[test]
fn test_encoder_mode_combo_validation() {
    let mut s = base_settings();
    s.encoder_type = EncoderType::AacAt;
    s.bitrate_mode = BitrateMode::Cvbr;
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());

    for encoder_type in [EncoderType::NativeAac, EncoderType::Auto] {
        s.encoder_type = encoder_type;
        s.bitrate_mode = BitrateMode::Cbr;
        assert!(validate_encoder_settings(&s).is_ok());
        s.bitrate_mode = BitrateMode::Cvbr;
        assert!(validate_encoder_settings(&s).is_err());
        s.bitrate_mode = BitrateMode::Vbr(100);
        assert!(validate_encoder_settings(&s).is_err());
    }

    s.encoder_type = EncoderType::Faac;
    s.bitrate_mode = BitrateMode::Abr;
    assert!(validate_encoder_settings(&s).is_ok());
    for quality in [0, 3, 5001] {
        s.bitrate_mode = BitrateMode::Vbr(quality);
        assert!(validate_encoder_settings(&s).is_err());
    }
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn output_path_validation_covers_extension_and_directory() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let valid_output = temp_dir.path().join("output.m4b");
    assert!(validate_output_target(&valid_output, AudiobookFormat::M4b).is_ok());

    let invalid_ext = temp_dir.path().join("output.mp3");
    let err = validate_output_target(&invalid_ext, AudiobookFormat::M4b)
        .expect_err("expected extension error");
    assert_eq!(code_of(&err), AppErrorCode::InvalidInput);

    let other_formats_extension = temp_dir.path().join("output.m4a");
    assert!(validate_output_target(&other_formats_extension, AudiobookFormat::M4b).is_err());
    assert!(validate_output_target(&other_formats_extension, AudiobookFormat::M4aOpus).is_ok());
    assert!(validate_output_target(
        &temp_dir.path().join("OUTPUT.MKA"),
        AudiobookFormat::MkaOpus
    )
    .is_ok());

    let missing_dir = PathBuf::from("/nonexistent/path/output.m4b");
    let err = validate_output_target(&missing_dir, AudiobookFormat::M4b)
        .expect_err("expected missing dir error");
    assert_eq!(code_of(&err), AppErrorCode::FileValidationFailed);
}

#[test]
fn output_path_validation_rejects_file_parent() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_parent = temp_dir.path().join("not_a_dir.txt");
    std::fs::write(&file_parent, b"test").expect("create temp file");

    let output_path = file_parent.join("child.m4b");
    let err = validate_output_target(&output_path, AudiobookFormat::M4b)
        .expect_err("expected not directory error");
    assert_eq!(code_of(&err), AppErrorCode::FileValidationFailed);
}
