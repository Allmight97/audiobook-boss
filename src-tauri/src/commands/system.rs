use crate::commands::CommandResult;

/// Simple ping command that returns "pong"
/// Used for testing basic Tauri command functionality
#[tauri::command]
#[specta::specta]
pub fn ping() -> CommandResult<String> {
    Ok("pong".to_string())
}

/// Echo command that returns the input string
/// Demonstrates parameter passing in Tauri commands
#[tauri::command]
#[specta::specta]
pub fn echo(input: String) -> CommandResult<String> {
    Ok(input)
}

// Removed get_ffmpeg_version command during nuclear transition (ffmpeg-next only).
