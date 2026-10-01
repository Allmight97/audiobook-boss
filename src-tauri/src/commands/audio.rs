use std::collections::HashMap;

use abb_engine::audio::SupportedAudioImportMetadata;
use abb_engine::processing::{ProcessCommandResult, ProcessPayload, ProcessingPreflightPlan};
use abb_engine::MetadataIntentPatch;

use crate::commands::{CommandResult, EngineState};

/// Returns backend-owned supported local audio import metadata for picker UI.
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_import_metadata(
    engine: EngineState<'_>,
) -> CommandResult<SupportedAudioImportMetadata> {
    Ok(engine.supported_audio_import_metadata())
}

#[tauri::command]
#[specta::specta]
pub fn preflight_processing_plan(
    engine: EngineState<'_>,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> CommandResult<ProcessingPreflightPlan> {
    Ok(engine.preflight_processing_plan(payload, metadata, preview_seconds)?)
}

/// Processes a direct preview with configurable encoder settings.
///
/// Final processing must enter through WorkRuntime so it has durable
/// operation identity, snapshots, and operation and title cancellation.
#[tauri::command]
#[specta::specta]
pub async fn process_audiobook_files(
    engine: EngineState<'_>,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> CommandResult<ProcessCommandResult> {
    Ok(engine
        .process_preview(payload, metadata, preview_seconds)
        .await?)
}
