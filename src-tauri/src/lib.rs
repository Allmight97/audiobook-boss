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
pub use metadata::{AlbumSortPatchOp, AudiobookMetadata, MetadataIntentPatch, PatchOp};

// Test-facing passthrough helpers used by integration tests.
pub use metadata::passthrough::{
    add_chapters_to_output, extract_passthrough_metadata, ChapterSpec,
};

pub mod audio;
pub use errors::{
    sanitize_path_for_display, sanitize_path_str_for_display, AppError, AppErrorCategory,
    AppErrorCode, AppErrorEnvelope,
};

use std::sync::Arc;
use tauri::{Manager, PhysicalSize, Size, WebviewWindow};

/// Type alias for managed JobRegistry state
pub type ManagedJobRegistry = Arc<audio::JobRegistry>;

const STARTUP_MAX_MONITOR_RATIO: f64 = 0.88;
const STARTUP_TARGET_ASPECT_RATIO: f64 = 16.0 / 10.0;
const STARTUP_PREFERRED_WIDTH: u32 = 1600;
const STARTUP_MIN_WIDTH: u32 = 1200;
const STARTUP_MIN_HEIGHT: u32 = 760;

fn monitor_fit_window_size(work_area_width: u32, work_area_height: u32) -> Option<(u32, u32)> {
    if work_area_width == 0 || work_area_height == 0 {
        return None;
    }

    let max_width = ((work_area_width as f64) * STARTUP_MAX_MONITOR_RATIO).floor() as u32;
    let max_height = ((work_area_height as f64) * STARTUP_MAX_MONITOR_RATIO).floor() as u32;

    if max_width == 0 || max_height == 0 {
        return None;
    }

    let mut width = max_width.min(STARTUP_PREFERRED_WIDTH);
    let mut height = ((width as f64) / STARTUP_TARGET_ASPECT_RATIO).round() as u32;

    if height > max_height {
        height = max_height;
        width = ((height as f64) * STARTUP_TARGET_ASPECT_RATIO).round() as u32;
    }

    width = width.min(max_width);
    height = height.min(max_height);

    if width < STARTUP_MIN_WIDTH || height < STARTUP_MIN_HEIGHT {
        width = max_width;
        height = max_height;
    }

    Some((width.max(1), height.max(1)))
}

fn configure_startup_window(window: &WebviewWindow) -> Result<(), tauri::Error> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let monitor_size = monitor.size();
    if let Some((width, height)) = monitor_fit_window_size(monitor_size.width, monitor_size.height)
    {
        window.set_size(Size::Physical(PhysicalSize::new(width, height)))?;
        window.center()?;
        log::info!(
            "Startup window fit to monitor: monitor={}x{}, window={}x{}",
            monitor_size.width,
            monitor_size.height,
            width,
            height
        );
    }

    Ok(())
}

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

            if let Some(main_window) = app.get_webview_window("main") {
                if let Err(error) = configure_startup_window(&main_window) {
                    log::warn!(
                        "Failed to fit startup window to monitor work area: {}",
                        error
                    );
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::monitor_fit_window_size;

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_returns_none_for_zero_inputs() {
        assert_eq!(monitor_fit_window_size(0, 1080), None);
        assert_eq!(monitor_fit_window_size(1920, 0), None);
    }

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_caps_to_monitor_budget() {
        let Some((width, height)) = monitor_fit_window_size(1920, 1080) else {
            panic!("expected window size");
        };

        assert!(width <= 1689);
        assert!(height <= 950);
        assert!(width >= 1200);
        assert!(height >= 760);
    }

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_uses_available_space_on_small_monitors() {
        let Some((width, height)) = monitor_fit_window_size(1024, 640) else {
            panic!("expected window size");
        };

        assert_eq!(width, 901);
        assert_eq!(height, 563);
    }
}
