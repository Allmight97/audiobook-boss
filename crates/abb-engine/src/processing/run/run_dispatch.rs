use super::run_job::{run_processing_job, supplemental_assets_for_input, ProcessingJobRequest};
use super::ProcessingRun;
use crate::audio;
use crate::errors::{AppError, Result};
use crate::host::Host;
use crate::output_artifact::OutputParentDirCleanup;
use crate::processing::plan::{ExecutionProcessingPlan, ResolvedProcessingPlan};
use crate::processing::terminal_outcomes::{
    batch_results, build_all_skipped_batch_result, no_write_skip_message, TitleOutcome,
};
use crate::processing::{JobIdentity, ProgressEventListener};
use crate::processing::{OperationKind, ProcessCommandResult, ProcessPayload, ProcessResultEntry};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use tokio_util::sync::CancellationToken;

pub(crate) async fn dispatch_title_jobs(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    execution_plan: ExecutionProcessingPlan,
    run: ProcessingRun,
) -> Result<ProcessCommandResult> {
    let ExecutionProcessingPlan {
        plan,
        file_info,
        output_parent_cleanup,
    } = execution_plan;
    let folders = Arc::new(Mutex::new(output_parent_cleanup));
    let batch = Batch {
        plan,
        file_info,
        folders: Arc::clone(&folders),
    };
    let result = dispatch_batch_plan(host, registry, workspace_root, payload, batch, run).await;
    let mut folders = folders.lock().unwrap_or_else(PoisonError::into_inner);
    crate::processing::output_parent_cleanup::finalize_output_parent_cleanup(result, &mut folders)
}

type TitleFuture = Pin<Box<dyn Future<Output = Result<ProcessResultEntry>> + Send>>;

struct Batch {
    plan: ResolvedProcessingPlan,
    file_info: audio::FileListInfo,
    folders: Arc<Mutex<OutputParentDirCleanup>>,
}

async fn dispatch_batch_plan(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: &ProcessPayload,
    batch: Batch,
    run: ProcessingRun,
) -> Result<ProcessCommandResult> {
    let Batch {
        plan,
        file_info,
        folders,
    } = batch;
    if let Some(result) = build_all_skipped_batch_result(&plan) {
        return Ok(result);
    }

    let mut scheduled_jobs: Vec<(TitleFuture, Option<CancellationToken>)> = Vec::new();
    for planned_job in plan.jobs {
        let input_index = planned_job.input_index;
        let title = run.mode.title(input_index)?;
        let identity = JobIdentity {
            operation_id: run.operation_id.clone(),
            job_id: None,
            input_index,
        };
        if let Some(message) = no_write_skip_message(&planned_job.output) {
            let entry = TitleOutcome::Skipped { message }.report(identity, &run.listener);
            if let Some(output) = title.output {
                output.end();
            }
            scheduled_jobs.push((Box::pin(async move { Ok(entry) }), None));
            continue;
        }

        let request = ProcessingJobRequest {
            host: host.clone(),
            registry: registry.clone(),
            workspace_root: workspace_root.clone(),
            audio_handling: planned_job.audio_plan.handling.clone(),
            audio_request: payload.audio_requests[input_index].clone(),
            audio_reason: planned_job.audio_plan.reason.clone(),
            metadata_intent: planned_job.metadata_intent.clone(),
            sample_rate: planned_job.audio_plan.sample_rate,
            aac_decoder: payload.accepted_aac_decoder()?,
            identity,
            operation_kind: OperationKind::ProcessingBatch,
            title_cancel: title.cancel.clone(),
            output_plan: planned_job.output.clone(),
            file_info: crate::processing::plan::title_file_info(
                &file_info,
                &planned_job.source_paths,
            )?,
            metadata: planned_job.metadata.clone(),
            cover_art_passthrough: planned_job.cover_art_passthrough,
            supplemental_assets: supplemental_assets_for_input(payload, input_index),
            progress_listener: run.listener.clone(),
            title_output: title.output,
        };
        let cancel = Some(request.title_cancel.clone());
        scheduled_jobs.push((
            Box::pin(run_title_job(request, Arc::clone(&folders))),
            cancel,
        ));
    }

    let outcomes = registry.run_batch(scheduled_jobs).await;
    let results = report_stopped_titles(outcomes, &run.operation_id, &run.listener);
    Ok(ProcessCommandResult::new(batch_results(results)?))
}

fn report_stopped_titles(
    outcomes: Vec<Result<ProcessResultEntry>>,
    operation_id: &str,
    listener: &ProgressEventListener,
) -> Vec<ProcessResultEntry> {
    outcomes
        .into_iter()
        .enumerate()
        .map(|(input_index, outcome)| {
            outcome.unwrap_or_else(|stopped| {
                let identity = JobIdentity {
                    operation_id: operation_id.to_string(),
                    job_id: None,
                    input_index,
                };
                TitleOutcome::from_error(None, stopped).report(identity, listener)
            })
        })
        .collect()
}

async fn run_title_job(
    request: ProcessingJobRequest,
    folders: Arc<Mutex<OutputParentDirCleanup>>,
) -> Result<ProcessResultEntry> {
    let listener = request.progress_listener.clone();
    let identity = request.identity.clone();
    let input_index = identity.input_index;
    let title_output = request.title_output.clone();
    let outcome = if request.title_cancel.is_cancelled() {
        TitleOutcome::from_error(None, AppError::cancelled())
    } else {
        run_processing_job(request).await
    };
    let mut folders = folders.lock().unwrap_or_else(PoisonError::into_inner);
    if let Err(error) = folders.end_title(input_index, outcome.is_published()) {
        log::warn!(
            "output_parent_cleanup status=title_cleanup_err input_index={input_index} err={error}"
        );
    }
    drop(folders);
    if let Some(title) = &title_output {
        title.end();
    }
    Ok(outcome.report(identity, &listener))
}

#[cfg(test)]
#[path = "run_dispatch_tests.rs"]
mod tests;
