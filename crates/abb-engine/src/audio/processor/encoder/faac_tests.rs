use super::*;

#[test]
fn rejects_clamped_he_bitrate_but_allows_lc_and_auto() {
    let mut settings = EncoderSettings {
        encoder_type: crate::audio::EncoderType::Faac,
        bitrate_kbps: 193,
        bitrate_mode: BitrateMode::Abr,
        channels: crate::audio::ChannelConfig::Stereo,
        native_aac_speed: 0,
        faac_profile: FaacProfile::HeAacV1,
    };
    assert!(FaacEncoder::open(32000, 2, &settings).is_err());
    settings.bitrate_kbps = 192;
    assert!(FaacEncoder::open(32000, 2, &settings).is_ok());
    settings.bitrate_kbps = 193;
    for profile in [FaacProfile::AacLc, FaacProfile::Auto] {
        settings.faac_profile = profile;
        let encoder = FaacEncoder::open(32000, 2, &settings)
            .expect("LC and Auto accept the target above the HE ceiling");
        assert!(!encoder.is_he());
        assert_eq!(encoder.info.bit_rate * 2, 193000);
    }
}
