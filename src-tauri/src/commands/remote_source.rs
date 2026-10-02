use abb_engine::remote_source::{
    ProviderId, RemoteAuthCompletionRequest, RemoteAuthStartResponse, RemoteLibraryResponse,
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
