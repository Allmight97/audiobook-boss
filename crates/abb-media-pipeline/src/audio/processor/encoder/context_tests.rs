use super::*;

#[test]
fn frame_plan_uses_reported_encoder_frame_size() {
    assert_eq!(
        samples_per_frame(2048, EncoderType::NativeAac).expect("reported frame size"),
        2048
    );
}

#[test]
fn frame_plan_uses_aac_quantum_for_variable_frame_encoders() {
    for encoder in [EncoderType::NativeAac, EncoderType::AacAt] {
        assert_eq!(
            samples_per_frame(0, encoder)
                .expect("resolved AAC encoder should have an explicit frame quantum"),
            AAC_FRAME_QUANTUM_SAMPLES
        );
    }
}

#[test]
fn frame_plan_rejects_unreported_frame_size_without_a_quantum() {
    for encoder in [EncoderType::Opus, EncoderType::Faac] {
        let err = samples_per_frame(0, encoder)
            .expect_err("encoders without an AAC quantum must report a frame size");
        assert!(err.to_string().contains("no fixed frame quantum"));
    }
}
