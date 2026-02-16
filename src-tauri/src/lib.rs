// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

pub mod commands;
mod errors;
pub mod ipc_contract;
mod metadata;
// Re-export key public types needed by external integration tests without exposing full internal module structure
pub use metadata::ffmpeg_bridge::{
    add_cover_art_stream_pre_header as ffmpeg_add_cover_art_stream_pre_header,
    detect_cover_art_format as ffmpeg_detect_cover_art_format,
    set_container_metadata as ffmpeg_set_container_metadata,
    validate_metadata_compatibility as ffmpeg_validate_metadata_compatibility,
    write_cover_art_packet_post_header as ffmpeg_write_cover_art_packet_post_header,
    CoverFormat as FfmpegCoverFormat,
};
pub use metadata::AudiobookMetadata;

// Test-facing passthrough helpers used by integration tests.
pub use metadata::passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, ChapterSpec,
};

pub mod audio;
pub use errors::{sanitize_path_for_display, sanitize_path_str_for_display};

use std::sync::Arc;

/// Type alias for managed JobRegistry state
pub type ManagedJobRegistry = Arc<audio::JobRegistry>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with INFO level for production
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting AudioBook Boss application");

    // Initialize job registry with auto-detected concurrency (num_cpus / 2)
    let job_registry: ManagedJobRegistry = Arc::new(audio::JobRegistry::auto());
    log::info!(
        "Job registry initialized: max_concurrent = {}",
        job_registry.max_concurrent()
    );

    let specta_builder = ipc_contract::builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(job_registry)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
