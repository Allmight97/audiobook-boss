use crate::commands::CommandResult;
use crate::remote_source::{
    AcquisitionJob, AcquisitionPlan, ProviderId, RemoteAuthCompletionRequest,
    RemoteAuthStartResponse, RemoteIndexerConnection, RemoteIndexerConnectionTestResult,
    RemoteIndexerConnectionUpdate, RemoteLibraryResponse, RemoteReleaseGrabRequest,
    RemoteReleaseGrabResponse, RemoteReleaseSearchRequest, RemoteReleaseSearchResponse,
    RemoteSourceAccountState, RemoteSourceProviderCapabilities,
};

#[tauri::command]
#[specta::specta]
pub fn list_remote_source_providers(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
) -> CommandResult<Vec<RemoteSourceProviderCapabilities>> {
    Ok(runtime.list_providers())
}

#[tauri::command]
#[specta::specta]
pub fn get_remote_source_account_state(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    provider_id: ProviderId,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(runtime.account_state(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn start_remote_source_auth(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    provider_id: ProviderId,
) -> CommandResult<RemoteAuthStartResponse> {
    Ok(runtime.start_auth(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn complete_remote_source_auth(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    request: RemoteAuthCompletionRequest,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(runtime.complete_auth(request).await?)
}

#[tauri::command]
#[specta::specta]
pub fn logout_remote_source_account(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    provider_id: ProviderId,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(runtime.logout(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn load_remote_source_library(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    provider_id: ProviderId,
) -> CommandResult<RemoteLibraryResponse> {
    Ok(runtime.load_library(provider_id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn start_remote_source_acquisition(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    plan: AcquisitionPlan,
) -> CommandResult<AcquisitionJob> {
    Ok(runtime.start_acquisition(plan).await?)
}

#[tauri::command]
#[specta::specta]
pub fn get_remote_source_acquisition_status(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    job_id: String,
) -> CommandResult<AcquisitionJob> {
    Ok(runtime.acquisition_status(&job_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_remote_source_acquisition(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    job_id: String,
) -> CommandResult<AcquisitionJob> {
    Ok(runtime.cancel_acquisition(&job_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn purge_remote_source_session(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    job_id: String,
) -> CommandResult<()> {
    Ok(runtime.purge_session(&job_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn search_remote_source_releases(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    request: RemoteReleaseSearchRequest,
) -> CommandResult<RemoteReleaseSearchResponse> {
    Ok(runtime.search_releases(request).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn grab_remote_source_release(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    request: RemoteReleaseGrabRequest,
) -> CommandResult<RemoteReleaseGrabResponse> {
    Ok(runtime.grab_release(request).await?)
}

#[tauri::command]
#[specta::specta]
pub fn get_remote_source_indexer_connection(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
) -> CommandResult<RemoteIndexerConnection> {
    Ok(runtime.get_indexer_connection()?)
}

#[tauri::command]
#[specta::specta]
pub fn update_remote_source_indexer_connection(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    update: RemoteIndexerConnectionUpdate,
) -> CommandResult<RemoteIndexerConnection> {
    Ok(runtime.update_indexer_connection(update)?)
}

#[tauri::command]
#[specta::specta]
pub async fn test_remote_source_indexer_connection(
    runtime: tauri::State<'_, crate::remote_source::RemoteSourceRuntime>,
    update: RemoteIndexerConnectionUpdate,
) -> CommandResult<RemoteIndexerConnectionTestResult> {
    Ok(runtime.test_indexer_connection(update).await?)
}
