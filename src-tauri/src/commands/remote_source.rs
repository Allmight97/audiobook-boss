use abb_engine::remote_source::{
    AcquisitionJob, AcquisitionPlan, ProviderId, RemoteAuthCompletionRequest,
    RemoteAuthStartResponse, RemoteIndexerConnection, RemoteIndexerConnectionTestResult,
    RemoteIndexerConnectionUpdate, RemoteLibraryResponse, RemoteReleaseGrabRequest,
    RemoteReleaseGrabResponse, RemoteReleaseSearchRequest, RemoteReleaseSearchResponse,
    RemoteSourceAccountState, RemoteSourceProviderCapabilities,
};

use crate::commands::{CommandResult, EngineState};

#[tauri::command]
#[specta::specta]
pub fn list_remote_source_providers(
    engine: EngineState<'_>,
) -> CommandResult<Vec<RemoteSourceProviderCapabilities>> {
    Ok(engine.remote_source().list_providers())
}

#[tauri::command]
#[specta::specta]
pub fn get_remote_source_account_state(
    engine: EngineState<'_>,
    provider_id: ProviderId,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(engine.remote_source().account_state(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn start_remote_source_auth(
    engine: EngineState<'_>,
    provider_id: ProviderId,
) -> CommandResult<RemoteAuthStartResponse> {
    Ok(engine.remote_source().start_auth(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn complete_remote_source_auth(
    engine: EngineState<'_>,
    request: RemoteAuthCompletionRequest,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(engine.remote_source().complete_auth(request).await?)
}

#[tauri::command]
#[specta::specta]
pub fn logout_remote_source_account(
    engine: EngineState<'_>,
    provider_id: ProviderId,
) -> CommandResult<RemoteSourceAccountState> {
    Ok(engine.remote_source().logout(provider_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn load_remote_source_library(
    engine: EngineState<'_>,
    provider_id: ProviderId,
) -> CommandResult<RemoteLibraryResponse> {
    Ok(engine.remote_source().load_library(provider_id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn start_remote_source_acquisition(
    engine: EngineState<'_>,
    plan: AcquisitionPlan,
) -> CommandResult<AcquisitionJob> {
    Ok(engine.remote_source().start_acquisition(plan).await?)
}

#[tauri::command]
#[specta::specta]
pub fn get_remote_source_acquisition_status(
    engine: EngineState<'_>,
    job_id: String,
) -> CommandResult<AcquisitionJob> {
    Ok(engine.remote_source().acquisition_status(&job_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_remote_source_acquisition(
    engine: EngineState<'_>,
    job_id: String,
) -> CommandResult<AcquisitionJob> {
    Ok(engine.remote_source().cancel_acquisition(&job_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn search_remote_source_releases(
    engine: EngineState<'_>,
    request: RemoteReleaseSearchRequest,
) -> CommandResult<RemoteReleaseSearchResponse> {
    Ok(engine.remote_source().search_releases(request).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn grab_remote_source_release(
    engine: EngineState<'_>,
    request: RemoteReleaseGrabRequest,
) -> CommandResult<RemoteReleaseGrabResponse> {
    Ok(engine.remote_source().grab_release(request).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_remote_source_indexer_connection(
    engine: EngineState<'_>,
) -> CommandResult<RemoteIndexerConnection> {
    Ok(engine.remote_source().get_indexer_connection().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn update_remote_source_indexer_connection(
    engine: EngineState<'_>,
    update: RemoteIndexerConnectionUpdate,
) -> CommandResult<RemoteIndexerConnection> {
    Ok(engine
        .remote_source()
        .update_indexer_connection(update)
        .await?)
}

#[tauri::command]
#[specta::specta]
pub async fn test_remote_source_indexer_connection(
    engine: EngineState<'_>,
    update: RemoteIndexerConnectionUpdate,
) -> CommandResult<RemoteIndexerConnectionTestResult> {
    Ok(engine
        .remote_source()
        .test_indexer_connection(update)
        .await?)
}
