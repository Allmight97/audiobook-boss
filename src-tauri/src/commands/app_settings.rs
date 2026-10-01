use abb_engine::RuntimeSettingsCapabilities;

use crate::commands::{CommandResult, EngineState};

/// Returns backend-owned runtime settings capabilities for UI controls.
#[tauri::command]
#[specta::specta]
pub async fn get_runtime_settings_capabilities(
    engine: EngineState<'_>,
) -> CommandResult<RuntimeSettingsCapabilities> {
    Ok(engine.runtime_settings_capabilities().await?)
}
