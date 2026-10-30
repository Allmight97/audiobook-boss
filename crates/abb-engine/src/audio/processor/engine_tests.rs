use super::*;
use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType};
use crate::processing::{OutputConfig, ProcessingSession};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn in_process_run_record_sanitizes_paths_and_names_the_encoder() {
    let settings = EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_mode: BitrateMode::Cvbr,
        channels: ChannelConfig::Stereo,
        ..EncoderSettings::default()
    };
    let plan = MediaProcessingPlan::new(
        PathBuf::from("/private/tmp/worker-output.m4b"),
        settings.clone(),
        SampleRateConfig::Explicit(44_100),
        vec![PathBuf::from("/private/input/Book One.m4b")],
        12.5,
    );
    let context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Explicit(44_100),
        OutputConfig::new(Path::new("/tmp/final/Book.m4b")),
    );
    let mut diagnostics = EncodingRunDiagnostics {
        timing: super::super::run_diagnostics::RunTiming::start(),
        opened_encoder: Some("aac_at".to_string()),
        opened_rate: Some(44_100),
        opened_channels: Some(2),
        input_facts: Vec::new(),
        encoder_details: None,
    };

    let formatted = format_in_process_run(&plan, &context, &diagnostics, &Ok(()));
    assert!(formatted.starts_with("--- in-process-encoder run "));
    assert!(formatted.contains("status=success"));
    assert!(formatted.contains("elapsed_monotonic_ms="));
    assert!(formatted.contains("opened_settings encoder=aac_at rate=44100 channels=2"));
    assert!(formatted.contains("target_duration_seconds=12.500"));
    assert!(formatted.contains("input[0] file=Book One.m4b"));
    assert!(formatted.contains("temp_output=worker-output.m4b"));
    assert!(formatted.ends_with("--- end in-process-encoder run ---\n\n"));
    assert!(!formatted.contains("/private/input"));
    assert!(!formatted.contains("/private/tmp"));

    diagnostics.opened_encoder = None;
    let formatted = format_in_process_run(
        &plan,
        &context,
        &diagnostics,
        &Err(AppError::General("boom".into())),
    );
    assert!(formatted.contains("status=failed"));
    assert!(formatted.contains("status_detail="));
    assert!(formatted.contains("opened_settings encoder=unknown rate=44100 channels=2"));
}
