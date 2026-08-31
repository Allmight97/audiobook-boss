use crate::audio;
use crate::commands::CommandResult;
use crate::errors::AppError;
use crate::metadata::{hydrate_intent_map, CoverStash, IpcMetadataIntentPatch};
use crate::processing::ProcessPayload;
use crate::work_runtime::{
    OperationId, OperationListSnapshot, OperationSnapshot, SubmitProcessingOperationRequest,
    WorkSubmissionAccepted,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Manager;

#[derive(Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename = "SubmitProcessingOperationRequest", rename_all = "camelCase")]
pub struct IpcSubmitProcessingOperationRequest {
    pub payload: ProcessPayload,
    pub metadata: Option<HashMap<String, IpcMetadataIntentPatch>>,
    pub preview_seconds: Option<f64>,
    pub title: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn submit_processing_operation(
    window: tauri::Window,
    runtime: tauri::State<'_, crate::work_runtime::WorkRuntime>,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    stash: tauri::State<'_, CoverStash>,
    request: IpcSubmitProcessingOperationRequest,
) -> CommandResult<WorkSubmissionAccepted> {
    if registry.is_global_cancelled() && registry.get_aggregate_status().await.total_jobs == 0 {
        registry.reset_global_cancel();
    }
    let cache_dir = window
        .app_handle()
        .path()
        .app_cache_dir()
        .map_err(|error| {
            AppError::General(format!(
                "Failed to resolve processing workspace root: {error}"
            ))
        })?;
    let workspace_root = audio::processing_workspace_root(&cache_dir);
    let request = SubmitProcessingOperationRequest {
        payload: request.payload,
        metadata: hydrate_intent_map(request.metadata, &stash)?,
        preview_seconds: request.preview_seconds,
        title: request.title,
    };

    Ok(runtime
        .submit_processing_operation(window, registry.inner().clone(), workspace_root, request)
        .await?)
}

#[tauri::command]
#[specta::specta]
pub fn list_work_operations(
    runtime: tauri::State<'_, crate::work_runtime::WorkRuntime>,
) -> CommandResult<OperationListSnapshot> {
    Ok(runtime.list_operations()?)
}

#[tauri::command]
#[specta::specta]
pub fn get_work_operation(
    runtime: tauri::State<'_, crate::work_runtime::WorkRuntime>,
    operation_id: OperationId,
) -> CommandResult<OperationSnapshot> {
    Ok(runtime.get_operation(operation_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_work_operation(
    window: tauri::Window,
    runtime: tauri::State<'_, crate::work_runtime::WorkRuntime>,
    operation_id: OperationId,
) -> CommandResult<OperationSnapshot> {
    Ok(runtime.cancel_operation(&window, operation_id)?)
}
