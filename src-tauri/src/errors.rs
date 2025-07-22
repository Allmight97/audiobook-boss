use thiserror::Error;
use crate::ffmpeg::FFmpegError;

/// Application-wide error type for structured error handling
#[derive(Error, Debug)]
pub enum AppError {
    #[error("FFmpeg operation failed: {0}")]
    FFmpeg(#[from] FFmpegError),
    
    #[error("File validation failed: {0}")]
    FileValidation(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Operation failed: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let error = AppError::InvalidInput("test".to_string());
        let error_string: String = error.into();
        assert!(error_string.contains("Invalid input: test"));
    }

    #[test]
    fn test_ffmpeg_error_conversion() {
        let ffmpeg_error = FFmpegError::BinaryNotFound;
        let app_error = AppError::from(ffmpeg_error);
        assert!(matches!(app_error, AppError::FFmpeg(_)));
    }
}
