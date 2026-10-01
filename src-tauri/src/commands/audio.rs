use abb_engine::audio::SupportedAudioImportMetadata;

use crate::commands::{CommandResult, EngineState};

/// Returns backend-owned supported local audio import metadata for picker UI.
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_import_metadata(
    engine: EngineState<'_>,
) -> CommandResult<SupportedAudioImportMetadata> {
    Ok(engine.supported_audio_import_metadata())
}
