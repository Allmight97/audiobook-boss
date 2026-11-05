use abb_engine::audio::{
    validate_encoder_settings, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
};

fn base_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Auto,
        native_aac_speed: 0,
        faac_profile: abb_engine::audio::FaacProfile::Auto,
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
fn faac_settings_wire_values_are_stable() {
    let value = serde_json::to_value(EncoderSettings {
        encoder_type: EncoderType::Faac,
        bitrate_mode: BitrateMode::Abr,
        ..base_settings()
    })
    .expect("FAAC settings should serialize");

    assert_eq!(value["encoderType"], "faac");
    assert_eq!(value["bitrateMode"]["mode"], "abr");
    assert_eq!(value["faacProfile"], "auto");
}
