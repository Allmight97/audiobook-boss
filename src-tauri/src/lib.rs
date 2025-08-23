// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

pub mod commands;
mod errors;
mod metadata;
// Re-export key public types needed by external integration tests without exposing full internal module structure
pub use metadata::AudiobookMetadata;
pub use metadata::ffmpeg_bridge::{
    set_container_metadata as ffmpeg_set_container_metadata,
    add_cover_art_stream_pre_header as ffmpeg_add_cover_art_stream_pre_header,
    write_cover_art_packet_post_header as ffmpeg_write_cover_art_packet_post_header,
    detect_cover_art_format as ffmpeg_detect_cover_art_format,
    validate_metadata_compatibility as ffmpeg_validate_metadata_compatibility,
    CoverFormat as FfmpegCoverFormat,
};
pub use metadata::writer::{
    write_cover_art as lofty_write_cover_art,
    write_metadata as lofty_write_metadata,
};
pub mod audio;

#[cfg(test)]
pub mod tests_integration;
pub mod tests_metadata_integration; // formerly feature-gated

use std::sync::{Arc, Mutex};
use audio::ProcessingProgress;

/// Shared state for tracking processing status and cancellation
#[derive(Default, Debug)]
pub struct ProcessingState {
    pub is_processing: Arc<Mutex<bool>>,
    pub is_cancelled: Arc<Mutex<bool>>,
    pub progress: Arc<Mutex<Option<ProcessingProgress>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with INFO level for production
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();
    
    log::info!("Starting Audiobook Boss application");
    
    let processing_state = ProcessingState::default();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(processing_state)
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::echo,
            commands::validate_files,
            // Removed get_ffmpeg_version & merge_audio_files (legacy shell commands) in nuclear cleanup
            commands::read_audio_metadata,
            commands::write_audio_metadata,
            commands::write_cover_art,
            commands::load_cover_art_file,
            commands::analyze_audio_files,
            commands::validate_audio_settings,
            commands::validate_encoder_settings_cmd,
            commands::process_audiobook_files,
            commands::process_audiobook_files_v2,
            commands::cancel_processing
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
