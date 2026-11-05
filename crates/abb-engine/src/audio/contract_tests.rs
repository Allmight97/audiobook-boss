use std::path::Path;

use crate::audio::{
    validate_encoder_settings, validate_input_audio_path, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType,
};

#[test]
fn audio_contract_rejects_missing_input_path() {
    let err =
        validate_input_audio_path(Path::new("definitely-not-a-real-audiobook-boss-input.m4b"))
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
        bitrate_kbps: 0,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Mono,
        native_aac_speed: 0,
        faac_profile: crate::audio::FaacProfile::Auto,
    };

    let err = validate_encoder_settings(&settings).expect_err("invalid bitrate should fail");
    assert!(
        !err.to_string().is_empty(),
        "encoder validation should return a concrete error"
    );
}

#[test]
fn mp3_requests_and_pass_through_plans_carry_no_encoder_settings() {
    use crate::audio::{AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest};
    use crate::processing::AudioHandling;
    let mp3 = TitleAudioRequest {
        output: OutputAudio::Mp3,
        intent: AudioIntent::Preserve,
        sample_rate: SampleRateConfig::Auto,
    };
    let wire = serde_json::json!({ "format": "mp3", "intent": "preserve", "sampleRate": "auto" });
    assert_eq!(serde_json::to_value(&mp3).expect("serialize"), wire);
    assert_eq!(
        serde_json::from_value::<TitleAudioRequest>(wire).expect("deserialize"),
        mp3
    );

    let m4b: TitleAudioRequest = serde_json::from_value(serde_json::json!({
        "format": "m4b",
        "settings": EncoderSettings::default(),
        "intent": "encode",
        "sampleRate": "auto",
    }))
    .expect("an M4B request carries its encoder settings");
    assert_eq!(
        m4b.output.encoder_settings(),
        Some(&EncoderSettings::default())
    );
    assert!(
        serde_json::from_value::<TitleAudioRequest>(serde_json::json!({
            "format": "m4b",
            "intent": "encode",
            "sampleRate": "auto",
        }))
        .is_err()
    );

    assert_eq!(
        serde_json::to_value(AudioHandling::Preserve).expect("serialize"),
        serde_json::json!({ "handling": "preserve" })
    );
}
