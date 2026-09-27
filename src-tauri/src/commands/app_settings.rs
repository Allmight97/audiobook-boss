use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::app_settings::{
    self, AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan, AppSettingsRecoveryResult,
};
use crate::commands::CommandResult;
use crate::errors::{AppError, Result};

fn app_settings_config_dir(app: &tauri::AppHandle) -> Result<PathBuf> {
    app.path().app_config_dir().map_err(|error| {
        AppError::General(format!("Failed to resolve app config directory: {error}"))
    })
}

/// Applies the runtime side of accepted settings. Every entry point calls this
/// only after storage succeeded: startup, update, reset, and recovery.
pub(crate) fn apply_settings_to_runtime(
    power: &crate::power::PowerManager,
    settings: &AppSettings,
) {
    power.set_enabled(settings.keep_awake_while_working);
    crate::audio::set_user_external_ffmpeg_path(
        settings
            .toolchain
            .external_ffmpeg_path
            .clone()
            .map(Into::into),
    );
}

#[tauri::command]
#[specta::specta]
pub fn get_app_settings(app: tauri::AppHandle) -> CommandResult<AppSettings> {
    let config_dir = app_settings_config_dir(&app)?;
    Ok(app_settings::get_app_settings(&config_dir)?)
}

#[tauri::command]
#[specta::specta]
pub fn get_app_settings_recovery(
    app: tauri::AppHandle,
) -> CommandResult<Option<AppSettingsRecoveryPlan>> {
    Ok(app_settings::get_app_settings_recovery(
        &app_settings_config_dir(&app)?,
    )?)
}

#[tauri::command]
#[specta::specta]
pub fn recover_app_settings(
    app: tauri::AppHandle,
    expected: AppSettingsRecoveryPlan,
) -> CommandResult<AppSettingsRecoveryResult> {
    let result = app_settings::recover_app_settings(&app_settings_config_dir(&app)?, expected)?;
    apply_settings_to_runtime(&app.state(), &result.settings);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn update_app_settings(
    app: tauri::AppHandle,
    patch: AppSettingsPatch,
) -> CommandResult<AppSettings> {
    let config_dir = app_settings_config_dir(&app)?;
    let settings = app_settings::update_app_settings(&config_dir, patch)?;
    apply_settings_to_runtime(&app.state(), &settings);
    Ok(settings)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_app_settings(
    app: tauri::AppHandle,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
) -> CommandResult<AppSettings> {
    let config_dir = app_settings_config_dir(&app)?;
    let settings = reset_app_settings_from_config_dir(&config_dir, &registry).await?;
    apply_settings_to_runtime(&app.state(), &settings);
    Ok(settings)
}

async fn reset_app_settings_from_config_dir(
    config_dir: &Path,
    registry: &crate::ManagedJobRegistry,
) -> Result<AppSettings> {
    let rollback_concurrency = registry.max_concurrent();
    registry.reset_to_auto().await?;

    let reset = app_settings::reset_app_settings(config_dir);
    if reset.is_err() {
        if let Err(rollback_error) = registry.update_max_concurrent(rollback_concurrency).await {
            log::warn!(
                "Failed to roll back max concurrency after settings reset failed: {}",
                rollback_error
            );
        }
    }
    reset
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::processing::JobRegistry;

    #[tokio::test]
    async fn failed_settings_reset_rolls_back_registry_concurrency() {
        let temp = TempDir::new().expect("temp dir");
        std::fs::create_dir(temp.path().join("app-settings.json"))
            .expect("create directory where settings file should be");
        let registry = Arc::new(JobRegistry::new(2));

        let error = reset_app_settings_from_config_dir(temp.path(), &registry)
            .await
            .expect_err("directory settings path should fail reset");

        assert!(matches!(error, AppError::Io(_)));
        assert_eq!(registry.max_concurrent(), 2);
    }
}
