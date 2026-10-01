use abb_engine::metadata_lookup::{MetadataLookupResponse, MetadataSource};
use abb_engine::{
    AudiobookMetadata, MetadataIntentPatch, MetadataIntentValidationResult,
    MetadataSaveBatchResult, MetadataSaveRequest,
};

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

/// Validates and normalizes metadata intent without writing files.
#[tauri::command]
#[specta::specta]
pub fn validate_metadata_intent_patch(
    engine: EngineState<'_>,
    metadata_patch: MetadataIntentPatch,
) -> CommandResult<MetadataIntentValidationResult> {
    Ok(engine.validate_metadata_intent_patch(&metadata_patch))
}

/// Returns the album sort (TSOA) processing would write for `metadata`.
#[tauri::command]
#[specta::specta]
pub fn preview_album_sort(
    engine: EngineState<'_>,
    metadata: AudiobookMetadata,
) -> CommandResult<Option<String>> {
    Ok(engine.preview_album_sort(&metadata))
}

/// Loads a cover image from disk and returns write-ready JPEG bytes.
#[tauri::command]
#[specta::specta]
pub async fn load_cover_art_file(
    engine: EngineState<'_>,
    file_path: String,
) -> CommandResult<Vec<u8>> {
    Ok(engine.load_cover_art_file(file_path).await?)
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

#[tauri::command]
#[specta::specta]
pub async fn save_metadata_batch(
    engine: EngineState<'_>,
    items: Vec<MetadataSaveRequest>,
) -> CommandResult<MetadataSaveBatchResult> {
    Ok(engine.save_metadata_batch(items).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn search_online_metadata(
    engine: EngineState<'_>,
    query: String,
    sources: Option<Vec<MetadataSource>>,
    limit: Option<u8>,
) -> CommandResult<MetadataLookupResponse> {
    Ok(engine.search_online_metadata(query, sources, limit).await?)
}
