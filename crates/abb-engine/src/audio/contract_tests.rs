use crate::audio::EncoderSettings;

#[test]
fn requests_carry_their_encoder_settings_and_pass_through_plans_carry_none() {
    use crate::audio::TitleAudioRequest;
    use crate::processing::AudioHandling;
    let m4b: TitleAudioRequest = serde_json::from_value(serde_json::json!({
        "format": "m4b",
        "settings": EncoderSettings::default(),
        "intent": "encode",
        "sampleRate": "auto",
    }))
    .expect("an M4B request carries its encoder settings");
    assert_eq!(m4b.output.encoder_settings(), &EncoderSettings::default());
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
