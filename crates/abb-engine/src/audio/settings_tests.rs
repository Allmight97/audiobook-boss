use super::*;

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
