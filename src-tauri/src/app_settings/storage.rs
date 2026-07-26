use std::io;
use std::path::{Path, PathBuf};

use crate::app_settings::types::{RAIL_WIDTH_MAX, RAIL_WIDTH_MIN};
use crate::app_settings::AppSettings;
use crate::errors::{AppError, Result};

const SETTINGS_FILE_NAME: &str = "app-settings.json";

pub(super) fn load(config_dir: &Path) -> Result<AppSettings> {
    let path = settings_path(config_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(AppSettings::default()),
        Err(error) => return Err(AppError::Io(error)),
    };

    let mut settings: AppSettings = serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidInput(format!(
            "App settings file is invalid. Reset app settings to restore defaults. ({error})"
        ))
    })?;
    // Layout preferences normalize on load (never fail): a hand-edited or
    // stale value must not brick launch hydration.
    settings.rail_width = settings.rail_width.clamp(RAIL_WIDTH_MIN, RAIL_WIDTH_MAX);
    Ok(settings)
}

pub(super) fn save(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let path = settings_path(config_dir);
    let temp_path = config_dir.join(format!(".app-settings-{}.tmp", uuid::Uuid::new_v4()));
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| AppError::General(format!("Failed to serialize app settings: {error}")))?;

    let write_and_replace = || -> Result<()> {
        std::fs::write(&temp_path, &content)?;

        if path.exists() {
            crate::file_replace::replace_file(&temp_path, &path)?;
        } else {
            std::fs::rename(&temp_path, &path)?;
        }

        Ok(())
    };

    let result = write_and_replace();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

pub(super) fn reset(config_dir: &Path) -> Result<()> {
    let path = settings_path(config_dir);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Io(error)),
    }
}

fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}
