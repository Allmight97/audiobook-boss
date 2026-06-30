use crate::commands::CommandResult;
use crate::remote_source::{
    AcquisitionJob, AcquisitionPlan, ProviderId, RemoteAuthCompletionRequest,
    RemoteAuthStartResponse, RemoteLibraryResponse, RemoteSourceAccountState,
    RemoteSourceProviderCapabilities,
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
