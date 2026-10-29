use abb_engine::audio::SupportedAudioImportMetadata;

use crate::commands::{CommandResult, EngineState};

#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_import_metadata(
    engine: EngineState<'_>,
) -> CommandResult<SupportedAudioImportMetadata> {
    Ok(engine.supported_audio_import_metadata())
}
