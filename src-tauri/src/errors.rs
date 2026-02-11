use std::path::Path;
use thiserror::Error;

/// Application-wide error type for structured error handling
#[derive(Error, Debug)]
pub enum AppError {
    #[error("File validation failed: {0}")]
    FileValidation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("FFmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),

    #[error("Process termination failed: {0}")]
    ProcessTermination(String),

    #[error("Temporary directory creation failed: {0}")]
    TempDirectoryCreation(String),

    #[error("Resource cleanup failed: {0}")]
    ResourceCleanup(String),

    #[error("Operation failed: {0}")]
    General(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

/// Sanitizes filesystem paths for user-facing messages.
/// Returns basename only and avoids exposing directory structure.
pub fn sanitize_path_for_display(path: &Path) -> String {
    path.file_name()
        .or_else(|| {
            path.components()
                .next_back()
                .map(|component| component.as_os_str())
        })
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "[path]".to_string())
}

/// Sanitizes string paths for user-facing messages.
pub fn sanitize_path_str_for_display(path: &str) -> String {
    sanitize_path_for_display(Path::new(path))
}

/// Convert AppError to string for Tauri command results
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

/// Convert AppError to Tauri InvokeError for command integration
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(error: AppError) -> Self {
        tauri::ipc::InvokeError::from_anyhow(anyhow::anyhow!(error))
    }
}

impl specta::Type for AppError {
    fn inline(
        type_map: &mut specta::TypeCollection,
        generics: specta::Generics,
    ) -> specta::datatype::DataType {
        <String as specta::Type>::inline(type_map, generics)
    }
}

// tests moved to `tests/unit/core/errors_tests.rs`
