use crate::audio::EncoderSettings;

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
