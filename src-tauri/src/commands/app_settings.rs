use abb_engine::app_settings::{
    AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan, AppSettingsRecoveryResult,
};
use abb_engine::RuntimeSettingsCapabilities;

use crate::commands::{CommandResult, EngineState};

#[tauri::command]
#[specta::specta]
pub fn get_app_settings(engine: EngineState<'_>) -> CommandResult<AppSettings> {
    Ok(engine.app_settings()?)
}

#[tauri::command]
#[specta::specta]
pub fn get_app_settings_recovery(
    engine: EngineState<'_>,
) -> CommandResult<Option<AppSettingsRecoveryPlan>> {
    Ok(engine.app_settings_recovery()?)
}

#[tauri::command]
#[specta::specta]
pub fn recover_app_settings(
    engine: EngineState<'_>,
    expected: AppSettingsRecoveryPlan,
) -> CommandResult<AppSettingsRecoveryResult> {
    Ok(engine.recover_app_settings(expected)?)
}

#[tauri::command]
#[specta::specta]
pub fn update_app_settings(
    engine: EngineState<'_>,
    patch: AppSettingsPatch,
) -> CommandResult<AppSettings> {
    Ok(engine.update_app_settings(patch)?)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_app_settings(engine: EngineState<'_>) -> CommandResult<AppSettings> {
    Ok(engine.reset_app_settings().await?)
}

/// Returns backend-owned runtime settings capabilities for UI controls.
#[tauri::command]
#[specta::specta]
pub async fn get_runtime_settings_capabilities(
    engine: EngineState<'_>,
) -> CommandResult<RuntimeSettingsCapabilities> {
    Ok(engine.runtime_settings_capabilities().await?)
}

/// Returns the current maximum concurrent jobs setting
#[tauri::command]
#[specta::specta]
pub fn get_max_concurrent_jobs(engine: EngineState<'_>) -> usize {
    engine.max_concurrent_jobs()
}

/// Updates the maximum concurrent jobs setting (requires idle state)
#[tauri::command]
#[specta::specta]
pub async fn set_max_concurrent_jobs(
    engine: EngineState<'_>,
    max_concurrent: Option<usize>,
) -> CommandResult<usize> {
    Ok(engine.set_max_concurrent_jobs(max_concurrent).await?)
}
