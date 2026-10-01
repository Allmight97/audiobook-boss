use std::collections::HashMap;

use abb_engine::audio::{
    AudiobookFormat, FileListInfo, SupportedAudioImportMetadata, TitleAudioPlan, TitleAudioRequest,
};
use abb_engine::output_artifact::{OutputKind, OutputNamingConfig};
use abb_engine::processing::{ProcessCommandResult, ProcessPayload, ProcessingPreflightPlan};
use abb_engine::{AudiobookMetadata, ChapterPlan, MetadataIntentPatch};

use crate::commands::{CommandResult, EngineState};

/// Validates and analyzes a list of audio files
/// Returns comprehensive file information including duration and size
#[tauri::command]
#[specta::specta]
pub fn analyze_audio_files(
    engine: EngineState<'_>,
    file_paths: Vec<String>,
) -> CommandResult<FileListInfo> {
    Ok(engine.analyze_audio_files(file_paths)?)
}

/// Returns backend-owned supported local audio import metadata for picker UI.
#[tauri::command]
#[specta::specta]
pub fn get_supported_audio_import_metadata(
    engine: EngineState<'_>,
) -> CommandResult<SupportedAudioImportMetadata> {
    Ok(engine.supported_audio_import_metadata())
}

/// Recursively discovers supported local audio files from files and directories.
#[tauri::command]
#[specta::specta]
pub async fn discover_audio_import_paths(
    engine: EngineState<'_>,
    input_paths: Vec<String>,
) -> CommandResult<Vec<String>> {
    Ok(engine.discover_audio_import_paths(input_paths).await?)
}

/// Drains local audio paths opened by the OS before the frontend was ready.
#[tauri::command]
#[specta::specta]
pub fn take_opened_audio_files(engine: EngineState<'_>) -> CommandResult<Vec<String>> {
    Ok(engine.take_opened_audio_files()?)
}

/// Builds an output path preview using backend naming rules without collision suffixing.
#[tauri::command]
#[specta::specta]
pub fn preview_output_path(
    engine: EngineState<'_>,
    output_dir: String,
    metadata: Option<AudiobookMetadata>,
    output_naming: Option<OutputNamingConfig>,
    source_path: Option<String>,
    output_kind: Option<OutputKind>,
    format: AudiobookFormat,
) -> CommandResult<String> {
    Ok(engine.preview_output_path(
        output_dir,
        metadata,
        output_naming,
        source_path,
        output_kind,
        format,
    )?)
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

#[tauri::command]
#[specta::specta]
pub async fn preview_title_audio(
    engine: EngineState<'_>,
    file_paths: Vec<String>,
    request: TitleAudioRequest,
    chapter_plans: Option<HashMap<String, ChapterPlan>>,
) -> CommandResult<TitleAudioPlan> {
    Ok(engine
        .preview_title_audio(file_paths, request, chapter_plans)
        .await?)
}
