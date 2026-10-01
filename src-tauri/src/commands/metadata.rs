use abb_engine::AudiobookMetadata;

use crate::commands::{CommandResult, EngineState};

/// Reads metadata from an audio file
/// Returns metadata as JSON-serializable struct
#[tauri::command]
#[specta::specta]
pub async fn read_audio_metadata(
    engine: EngineState<'_>,
    file_path: String,
) -> CommandResult<AudiobookMetadata> {
    Ok(engine.read_audio_metadata(file_path).await?)
}

/// Reads an audio file's embedded cover as a bounded JPEG thumbnail.
#[tauri::command]
#[specta::specta]
pub async fn read_audio_cover_thumbnail(
    engine: EngineState<'_>,
    file_path: String,
) -> CommandResult<Option<Vec<u8>>> {
    Ok(engine.read_audio_cover_thumbnail(file_path).await?)
}

/// Loads cover art from a remote HTTPS URL and returns write-ready JPEG bytes.
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_from_url(
    engine: EngineState<'_>,
    url: String,
) -> CommandResult<Vec<u8>> {
    Ok(engine.load_cover_art_from_url(url).await?)
}
