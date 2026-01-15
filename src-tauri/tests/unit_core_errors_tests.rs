//! Unit tests for core error types.
//!
//! NOTE: These tests are currently disabled because they test private error types
//! (AppError, FFmpegError) that are not part of the public API.
//!
//! TODO: Move these tests into src-tauri/src/errors.rs as module-level tests with #[cfg(test)].

// All tests below are commented out because they depend on private APIs

/*
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
*/

#[test]
fn placeholder_test_to_make_file_compile() {
    // This test exists only to make the file compile with no real tests.
    // Empty test body is intentional - tests are disabled pending refactor.
}
