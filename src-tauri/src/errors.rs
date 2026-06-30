use std::{fmt, path::Path};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCategory {
    Validation,
    Cancellation,
    Toolchain,
    Processing,
    Resource,
    Io,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    FileValidationFailed,
    InvalidInput,
    IoError,
    FfmpegError,
    ProcessTerminationFailed,
    TempDirectoryCreationFailed,
    ResourceCleanupFailed,
    InternalError,
    ImageProcessingError,
    ProcessingCancelled,
    ToolchainRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorEnvelope {
    pub code: AppErrorCode,
    pub category: AppErrorCategory,
    pub message: String,
    pub detail: Option<String>,
}

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

    #[error("{0}")]
    Cancellation(String),

    #[error("{0}")]
    ToolchainRequired(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn cancelled() -> Self {
        Self::Cancellation("Processing was cancelled".to_string())
    }

    pub fn toolchain_required(message: impl Into<String>) -> Self {
        Self::ToolchainRequired(message.into())
    }
}

impl From<AppError> for AppErrorEnvelope {
    fn from(error: AppError) -> Self {
        match error {
            AppError::FileValidation(message) => Self::new(
                AppErrorCode::FileValidationFailed,
                AppErrorCategory::Validation,
                format!("File validation failed: {message}"),
                None,
            ),
            AppError::InvalidInput(message) => Self::new(
                AppErrorCode::InvalidInput,
                AppErrorCategory::Validation,
                format!("Invalid input: {message}"),
                None,
            ),
            AppError::Io(error) => Self::new(
                AppErrorCode::IoError,
                AppErrorCategory::Io,
                format!("IO operation failed: {}", error),
                Some(error.to_string()),
            ),
            AppError::Ffmpeg(error) => Self::new(
                AppErrorCode::FfmpegError,
                AppErrorCategory::Toolchain,
                format!("FFmpeg error: {}", error),
                Some(error.to_string()),
            ),
            AppError::ProcessTermination(message) => Self::new(
                AppErrorCode::ProcessTerminationFailed,
                AppErrorCategory::Processing,
                format!("Process termination failed: {message}"),
                None,
            ),
            AppError::TempDirectoryCreation(message) => Self::new(
                AppErrorCode::TempDirectoryCreationFailed,
                AppErrorCategory::Resource,
                format!("Temporary directory creation failed: {message}"),
                None,
            ),
            AppError::ResourceCleanup(message) => Self::new(
                AppErrorCode::ResourceCleanupFailed,
                AppErrorCategory::Resource,
                format!("Resource cleanup failed: {message}"),
                None,
            ),
            AppError::General(message) => Self::new(
                AppErrorCode::InternalError,
                AppErrorCategory::Internal,
                format!("Operation failed: {message}"),
                None,
            ),
            AppError::ImageProcessing(message) => Self::new(
                AppErrorCode::ImageProcessingError,
                AppErrorCategory::Processing,
                format!("Image processing error: {message}"),
                None,
            ),
            AppError::Cancellation(message) => Self::new(
                AppErrorCode::ProcessingCancelled,
                AppErrorCategory::Cancellation,
                message,
                None,
            ),
            AppError::ToolchainRequired(message) => Self::new(
                AppErrorCode::ToolchainRequired,
                AppErrorCategory::Toolchain,
                message,
                None,
            ),
        }
    }
}

impl AppErrorEnvelope {
    pub fn new(
        code: AppErrorCode,
        category: AppErrorCategory,
        message: impl Into<String>,
        detail: Option<String>,
    ) -> Self {
        Self {
            code,
            category,
            message: message.into(),
            detail,
        }
    }
}

impl fmt::Display for AppErrorEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

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

/// Convert AppError to Tauri InvokeError for command integration
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(error: AppError) -> Self {
        tauri::ipc::InvokeError::from(AppErrorEnvelope::from(error))
    }
}

impl specta::Type for AppError {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        <String as specta::Type>::definition(types)
    }
}

// tests moved to `tests/unit/core/errors_tests.rs`
