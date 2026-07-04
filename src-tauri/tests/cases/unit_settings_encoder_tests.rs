//! Unit tests for encoder settings validation and resolution.
//!
//! Extracted from settings_encoder.rs to keep production code clean.

use audiobook_boss_lib::audio::{
    detect_encoder_availability, resolve_encoder_name, resolve_encoder_type,
    validate_encoder_settings, validate_requested_encoder_available, BitrateMode, ChannelConfig,
    EncoderAvailability, EncoderCapabilitySource, EncoderSettings, EncoderType,
    VALID_ENCODER_BITRATES,
};

fn base_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: ChannelConfig::Auto,
        afterburner: true,
    }
}

#[test]
fn test_validate_bitrate_whitelist() {
    for &br in VALID_ENCODER_BITRATES {
        let mut s = base_settings();
        s.bitrate_kbps = br;
        assert!(validate_encoder_settings(&s).is_ok());
    }
    // Test invalid bitrates (outside expanded 48-128 range)
    let mut s = base_settings();
    s.bitrate_kbps = 32; // below minimum
    assert!(validate_encoder_settings(&s).is_err());

    s.bitrate_kbps = 192; // above maximum
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_bitrate_mode_validation() {
    let mut s = base_settings();
    s.bitrate_mode = BitrateMode::Vbr(5);
    assert!(validate_encoder_settings(&s).is_ok());

    s.bitrate_mode = BitrateMode::Vbr(0);
    assert!(validate_encoder_settings(&s).is_err());

    s.bitrate_mode = BitrateMode::Vbr(6);
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_encoder_mode_combo_validation() {
    let mut s = base_settings();
    s.encoder_type = EncoderType::FdkHeAac;
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_ok());

    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());

    s.encoder_type = EncoderType::AacAt;
    s.bitrate_mode = BitrateMode::Cvbr;
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());

    s.encoder_type = EncoderType::NativeAac;
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_err());

    // Auto remains VBR-only
    s.encoder_type = EncoderType::Auto;
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Cvbr;
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_detect_available_encoders_struct_defaults() {
    // We can't guarantee availability on CI, but the function should always return a struct.
    let availability = detect_encoder_availability();
    assert!(
        availability.native_aac_available
            || availability.aac_at_available
            || availability.fdk_available,
        "At least one encoder should be available in the environment"
    );
}

#[test]
fn test_resolve_encoder_type_prefers_available() {
    let availability = EncoderAvailability {
        fdk_available: true,
        fdk_source: EncoderCapabilitySource::Detected,
        aac_at_available: true,
        native_aac_available: true,
        auto_encoder: EncoderType::FdkHeAac,
        detected_toolchain_path: Some("/opt/homebrew/bin/ffmpeg".into()),
        status_message: "FDK AAC detected and ready.".into(),
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability,
    );
    assert_eq!(resolved, EncoderType::FdkHeAac);

    let availability_no_fdk = EncoderAvailability {
        fdk_available: false,
        fdk_source: EncoderCapabilitySource::None,
        aac_at_available: true,
        native_aac_available: true,
        auto_encoder: EncoderType::AacAt,
        detected_toolchain_path: None,
        status_message: "No external FFmpeg toolchain with libfdk_aac was detected.".into(),
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability_no_fdk,
    );
    assert_eq!(resolved, EncoderType::AacAt);

    let availability_none = EncoderAvailability {
        fdk_available: false,
        fdk_source: EncoderCapabilitySource::None,
        aac_at_available: false,
        native_aac_available: true,
        auto_encoder: EncoderType::NativeAac,
        detected_toolchain_path: None,
        status_message: "No external FFmpeg toolchain with libfdk_aac was detected.".into(),
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability_none,
    );
    assert_eq!(resolved, EncoderType::NativeAac);
}

#[test]
#[should_panic(expected = "resolved encoder type")]
fn test_resolve_encoder_name_rejects_auto() {
    let _ = resolve_encoder_name(EncoderType::Auto);
}

#[test]
fn test_validate_requested_encoder_available_rejects_unavailable_explicit_encoders() {
    let availability = EncoderAvailability {
        fdk_available: false,
        fdk_source: EncoderCapabilitySource::None,
        aac_at_available: false,
        native_aac_available: false,
        auto_encoder: EncoderType::NativeAac,
        detected_toolchain_path: None,
        status_message: "No external FFmpeg toolchain with libfdk_aac was detected.".into(),
    };

    let fdk_error =
        validate_requested_encoder_available(EncoderType::FdkHeAac, &availability).unwrap_err();
    assert!(matches!(
        &fdk_error,
        audiobook_boss_lib::AppError::ToolchainRequired(_)
    ));
    assert_eq!(
        fdk_error.to_string(),
        "FDK AAC requires a validated external FFmpeg toolchain."
    );

    let aac_at_error =
        validate_requested_encoder_available(EncoderType::AacAt, &availability).unwrap_err();
    #[cfg(target_os = "macos")]
    assert_eq!(
        aac_at_error.to_string(),
        "Invalid input: Apple AAC is unavailable in this build."
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        aac_at_error.to_string(),
        "Invalid input: Apple AAC (aac_at) is only available on macOS."
    );

    let native_error =
        validate_requested_encoder_available(EncoderType::NativeAac, &availability).unwrap_err();
    assert_eq!(
        native_error.to_string(),
        "Invalid input: Native AAC (FFmpeg) is unavailable in this build."
    );
}

#[test]
fn test_validate_requested_encoder_available_allows_auto_resolution() {
    let availability = EncoderAvailability {
        fdk_available: false,
        fdk_source: EncoderCapabilitySource::None,
        aac_at_available: false,
        native_aac_available: true,
        auto_encoder: EncoderType::NativeAac,
        detected_toolchain_path: None,
        status_message: "Native AAC available.".into(),
    };

    validate_requested_encoder_available(EncoderType::Auto, &availability)
        .expect("auto should remain dynamically resolvable");
}
