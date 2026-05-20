use super::complete_staged_output;
use crate::audio::{
    BitrateMode, ChannelConfig, CleanupGuard, EncoderSettings, EncoderType, SampleRateConfig,
    ThreadSetting,
};
use crate::processing::{JobRegistry, OutputConfig, ProcessingContext, ProcessingSession};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: ChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

#[tokio::test]
async fn complete_staged_output_cleans_without_committing_when_cancelled_before_commit() {
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register job");
    let checker = registry.cancellation_checker(job_id).await;
    registry.cancel_job(job_id).await.expect("cancel job");

    let temp_dir = TempDir::new().expect("temp dir");
    let staged_output = temp_dir.path().join("staged.m4b");
    let final_output = temp_dir.path().join("final.m4b");
    fs::write(&staged_output, b"audio").expect("write staged output");

    let context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::from_job_registry(job_id.0, checker)),
        encoder_settings(),
        SampleRateConfig::Auto,
        OutputConfig::new(&final_output),
    );
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&staged_output);

    let error = complete_staged_output(&context, staged_output.clone(), &mut cleanup_guard, None)
        .expect_err("cancelled job should not commit staged output");

    assert!(
        error.to_string().contains("Processing was cancelled"),
        "unexpected error: {error}"
    );
    assert!(
        !final_output.exists(),
        "cancelled staged output should not be committed"
    );
    drop(cleanup_guard);
    assert!(
        !staged_output.exists(),
        "cancelled staged output should be cleaned"
    );
}
