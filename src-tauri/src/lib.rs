// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

mod commands;
mod errors;
mod ffmpeg;
mod metadata;
mod audio;

// Remove demo greet command - keep only production code

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::echo,
            commands::validate_files,
            commands::get_ffmpeg_version,
            commands::merge_audio_files,
            commands::read_audio_metadata,
            commands::write_audio_metadata,
            commands::write_cover_art,
            commands::analyze_audio_files,
            commands::validate_audio_settings,
            commands::process_audiobook_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
