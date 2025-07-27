// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

mod commands;
mod errors;
mod ffmpeg;
mod metadata;
mod audio;

#[cfg(test)]
mod tests_integration;

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
            commands::get_ffmpeg_version,
            commands::merge_audio_files,
            commands::read_audio_metadata,
            commands::write_audio_metadata,
            commands::write_cover_art,
            commands::load_cover_art_file,
            commands::analyze_audio_files,
            commands::validate_audio_settings,
            commands::process_audiobook_files,
            commands::cancel_processing
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
