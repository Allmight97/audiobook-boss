use std::path::PathBuf;

use crate::audio::constants::TEMP_MERGED_FILENAME;
use crate::audio::{AudioFile, EncoderSettings};
use crate::errors::{AppError, Result};
use crate::execution::ProcessingContext;

use super::engine;
use super::plan::MediaProcessingPlan;
use super::ProcessingWorkflow;

pub(super) fn encode(
    context: &ProcessingContext,
    settings: &EncoderSettings,
    workflow: &ProcessingWorkflow,
    files: &[AudioFile],
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
) -> Result<PathBuf> {
    log::info!(
        "Starting FFmpeg merge - Total duration: {:.2}s, Bitrate: {}k",
        workflow.total_duration,
        settings.bitrate_kbps
    );

    let temp_output = workflow
        .workspace
        .dir()
        .join(TEMP_MERGED_FILENAME)
        .with_extension(context.output.final_path().extension().unwrap_or_default());

    let file_paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let plan = MediaProcessingPlan::new(
        temp_output.clone(),
        settings.clone(),
        context.sample_rate,
        file_paths,
        workflow.total_duration,
    );

    log::info!(
        "media_job session_id={} job_id={} artifact={} encoder={:?}",
        context.session.id(),
        context.identity.job_id.as_deref().unwrap_or("unscoped"),
        crate::diagnostics::artifact_id(&temp_output),
        settings.encoder_type
    );
    crate::diagnostics::stage("encode_mux", &temp_output, || {
        engine::execute(&plan, context, metadata, passthrough)
    })?;

    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }

    Ok(temp_output)
}
