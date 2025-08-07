use audiobook_boss_lib::errors::AppError;
use audiobook_boss_lib::ffmpeg::FFmpegError;

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


