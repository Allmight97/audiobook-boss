use super::*;
use crate::audio::processor::encoder::context::create_audio_encoder;
use crate::audio::settings_encoder::{BitrateMode, ChannelConfig, EncoderType};

#[test]
fn opened_nmr_honors_requested_speed_and_target() {
    ff::init().expect("initialize FFmpeg");
    for (speed, bitrate_kbps) in [(0, 33), (4, 193)] {
        let mut settings = EncoderSettings {
            encoder_type: EncoderType::NativeAac,
            bitrate_kbps,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Stereo,
            native_aac_speed: speed,
            faac_profile: crate::audio::FaacProfile::Auto,
        };
        let encoder =
            create_audio_encoder(&settings, 44100, 2, true).expect("open requested NMR encoder");
        validate_native_options(&encoder, &settings).expect("verify opened NMR settings");
        assert_eq!(
            native_option(&encoder, c"aac_is").expect("read opened psychoacoustic option"),
            1
        );
        assert_eq!(
            native_option(&encoder, c"aac_pns").expect("read opened psychoacoustic option"),
            1
        );
        settings.native_aac_speed = (speed + 1) % 5;
        assert!(validate_native_options(&encoder, &settings).is_err());
        settings.native_aac_speed = speed;
        settings.bitrate_kbps += 1;
        assert!(validate_native_options(&encoder, &settings).is_err());
    }
}

#[test]
fn native_rejects_target_that_ffmpeg_would_silently_clamp() {
    let settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 133,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        native_aac_speed: 0,
        faac_profile: crate::audio::FaacProfile::Auto,
    };
    let error = create_audio_encoder(&settings, 22050, 1, true)
        .err()
        .expect("reject a target above the resolved mono ceiling");
    assert!(
        matches!(error, AppError::InvalidInput(message) if message.contains("Native target bitrate exceeds 132 kbps"))
    );
}
