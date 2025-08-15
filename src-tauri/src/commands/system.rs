use crate::errors::Result;
#[cfg(not(feature = "safe-ffmpeg"))]
use crate::ffmpeg;

/// Simple ping command that returns "pong"
/// Used for testing basic Tauri command functionality
#[tauri::command]
pub fn ping() -> Result<String> {
    Ok("pong".to_string())
}

/// Echo command that returns the input string
/// Demonstrates parameter passing in Tauri commands
#[tauri::command]
pub fn echo(input: String) -> Result<String> {
    Ok(input)
}

/// Get FFmpeg version information
/// Returns version string if FFmpeg is available
#[cfg(not(feature = "safe-ffmpeg"))]
#[tauri::command]
pub fn get_ffmpeg_version() -> Result<String> {
    Ok(ffmpeg::command::FFmpegCommand::version()?)
}

#[cfg(feature = "safe-ffmpeg")]
#[tauri::command]
pub fn get_ffmpeg_version() -> Result<String> {
    Ok("safe-ffmpeg mode (using ffmpeg-next library)".to_string())
}


