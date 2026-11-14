use crate::errors::{AppError, AppErrorCategory, AppErrorEnvelope, Result};
use crate::host::Host;
use crate::output_artifact::OutputFolderRecords;
use crate::processing::plan::{
    prepare_inspected_execution, resolve_processing_plan, title_file_info, InspectedProcessingPlan,
};
use crate::processing::title_output::{TitleOutput, TitleOutputPlan};
use crate::processing::{ProcessCommandResult, ProcessPayload};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod run_dispatch;
mod run_job;
mod run_options;
mod run_validation;

pub(crate) use run_job::supplemental_assets_for_input;
pub(crate) use run_options::{ExportTitle, ProcessingRun, RunMode};
use run_validation::inspect_and_validate_external_processing_contract;

#[derive(Debug, Clone)]
pub(crate) struct ProcessingRoots {
    pub(crate) workspace: PathBuf,
    pub(crate) output_folders: OutputFolderRecords,
}

impl ProcessingRoots {
    pub(crate) fn for_cache(cache_dir: &Path) -> Self {
        Self {
            workspace: crate::audio::processing_workspace_root(cache_dir),
            output_folders: OutputFolderRecords::for_cache(cache_dir),
        }
    }
}

/// Runs an inspected plan. `_run` is the caller's hold on the job registry,
/// required so concurrency cannot change under the run.
pub(crate) async fn process_inspected_with_options(
    _run: &crate::processing::job_registry::ActiveRun,
    host: Host,
    registry: crate::ManagedJobRegistry,
    roots: ProcessingRoots,
    payload: ProcessPayload,
    inspected: InspectedProcessingPlan,
    run: ProcessingRun,
) -> Result<ProcessCommandResult> {
    let result = async {
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by process_inspected_with_options"
        )]
        let execution = tokio::task::spawn_blocking(move || {
            prepare_inspected_execution(&roots.output_folders, &payload, inspected)
                .map(|plan| (payload, plan))
        })
        .await
        .map_err(|error| AppError::General(format!("Execution preparation failed: {error}")))??;
        run_dispatch::dispatch_title_jobs(
            host,
            registry,
            roots.workspace,
            &execution.0,
            execution.1,
            run,
        )
        .await
    }
    .await;
    if let Some(record) = result
        .as_ref()
        .err()
        .and_then(processing_request_rejected_record)
    {
        log::error!("{record}");
    }
    result
}

pub(crate) fn inspect_processing_plan(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview: bool,
) -> Result<InspectedProcessingPlan> {
    let file_info = inspect_and_validate_external_processing_contract(payload)?;
    let plan = resolve_processing_plan(payload, metadata, preview, &file_info)?;
    Ok(InspectedProcessingPlan { plan, file_info })
}

/// Stable dev-log diagnostic for processing requests that terminate as an
/// error before any job lifecycle exists (validation, planning, registration).
/// Counted by `scripts/dev-log-analysis.ts` via its generic ERROR counter;
/// cancellations are excluded so a user cancel cannot degrade a session.
fn processing_request_rejected_record(error: &AppError) -> Option<String> {
    let envelope = AppErrorEnvelope::from(error);
    if envelope.category == AppErrorCategory::Cancellation {
        return None;
    }
    Some(format!(
        "processing_request event=rejected code={} category={}",
        envelope.code.log_label(),
        envelope.category.log_label(),
    ))
}

pub(crate) fn preflight_title_outputs(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
    inspected: &InspectedProcessingPlan,
) -> Result<Vec<Arc<TitleOutput>>> {
    inspected
        .plan
        .jobs
        .iter()
        .map(|job| {
            let sources = title_file_info(&inspected.file_info, &job.source_paths)?.files;
            TitleOutput::new(TitleOutputPlan {
                anchor: job.input_path.clone(),
                sources: crate::audio::passthrough_sources_from_audio_files(&sources),
                base: job.source_metadata.clone(),
                accepted: metadata
                    .and_then(|map| map.get(&payload.input_files[job.input_index]))
                    .cloned(),
                output_dir: PathBuf::from(&payload.output_dir),
                naming: payload.output_naming.clone().unwrap_or_default(),
                extension: job.audio_plan.format.extension().to_string(),
                requested: job.output.requested_path.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
