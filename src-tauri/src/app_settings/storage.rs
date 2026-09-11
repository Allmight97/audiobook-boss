use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::app_settings::{
    AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan, AppSettingsRecoveryResult,
    EncoderDefaults, EncoderDefaultsScope, IncompatibleEncoderDefaults,
};
use crate::audio::EncoderType;
use crate::errors::{AppError, Result};

const SETTINGS_FILE_NAME: &str = "app-settings.json";

pub(super) fn load(config_dir: &Path) -> Result<AppSettings> {
    let path = settings_path(config_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(AppSettings::default()),
        Err(error) => return Err(AppError::Io(error)),
    };

    serde_json::from_str(&content).map_err(|error| {
        AppError::InvalidInput(format!(
            "App settings file could not be read by this version. Open App Settings to check recovery options, or reset app settings to restore defaults. ({error})"
        ))
    })
}

pub(super) fn save(config_dir: &Path, settings: &AppSettings) -> Result<()> {
    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| AppError::General(format!("Failed to serialize app settings: {error}")))?;
    save_content(config_dir, &content)
}

fn save_content(config_dir: &Path, content: &str) -> Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let path = settings_path(config_dir);
    let temp_path = config_dir.join(format!(".app-settings-{}.tmp", uuid::Uuid::new_v4()));
    let write_and_replace = || -> Result<()> {
        std::fs::write(&temp_path, content)?;

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

pub(super) fn recovery_plan(config_dir: &Path) -> Result<Option<AppSettingsRecoveryPlan>> {
    let content = match std::fs::read_to_string(settings_path(config_dir)) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    Ok(plan_recovery(&content)?.map(|(plan, _)| plan))
}

fn plan_recovery(content: &str) -> Result<Option<(AppSettingsRecoveryPlan, serde_json::Value)>> {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Ok(None);
    };
    let mut incompatible_encoders = Vec::new();
    for (pointer, scope) in [
        ("/encoderDefaults", EncoderDefaultsScope::LastUsed),
        (
            "/pinnedDefaults/encoderDefaults",
            EncoderDefaultsScope::Pinned,
        ),
    ] {
        let Some(defaults) = value.pointer_mut(pointer) else {
            continue;
        };
        let Some(encoder) = defaults
            .pointer("/settings/encoderType")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        if serde_json::from_value::<EncoderType>(serde_json::Value::String(encoder.to_owned()))
            .is_ok()
        {
            continue;
        }
        incompatible_encoders.push(IncompatibleEncoderDefaults {
            scope,
            encoder_type: encoder.to_owned(),
        });
        *defaults = serde_json::to_value(EncoderDefaults::default()).map_err(|error| {
            AppError::General(format!("Failed to prepare settings recovery: {error}"))
        })?;
    }
    if incompatible_encoders.is_empty() {
        return Ok(None);
    }
    // Offer recovery only when replacing the named groups restores valid settings.
    // Unrelated damage stays explicit and the original file remains untouched.
    let Ok(settings) = serde_json::from_value::<AppSettings>(value.clone()) else {
        return Ok(None);
    };
    if settings.merge(AppSettingsPatch::default()).is_err() {
        return Ok(None);
    }
    Ok(Some((
        AppSettingsRecoveryPlan {
            incompatible_encoders,
        },
        value,
    )))
}

pub(super) fn recover(
    config_dir: &Path,
    expected: AppSettingsRecoveryPlan,
) -> Result<AppSettingsRecoveryResult> {
    let content = std::fs::read_to_string(settings_path(config_dir))?;
    let Some((plan, recovered)) = plan_recovery(&content)? else {
        return Err(AppError::InvalidInput("These settings cannot be recovered by resetting unsupported encoders. Reopen App Settings to inspect the current file.".to_string()));
    };
    if plan != expected {
        return Err(AppError::InvalidInput(
            "Saved encoder defaults changed. Reopen App Settings to review recovery again."
                .to_string(),
        ));
    }
    let settings = serde_json::from_value::<AppSettings>(recovered.clone()).map_err(|error| {
        AppError::General(format!("Failed to prepare recovered settings: {error}"))
    })?;
    let recovered = serde_json::to_string_pretty(&recovered).map_err(|error| {
        AppError::General(format!("Failed to serialize recovered settings: {error}"))
    })?;
    let backup_file_name = format!("app-settings.before-recovery-{}.json", uuid::Uuid::new_v4());
    let mut backup = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(config_dir.join(&backup_file_name))?;
    backup.write_all(content.as_bytes())?;
    backup.sync_all()?;
    save_content(config_dir, &recovered)?;
    Ok(AppSettingsRecoveryResult {
        backup_file_name,
        settings,
    })
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
