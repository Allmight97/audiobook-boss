// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

pub mod app_settings;
pub mod commands;
mod errors;
mod file_replace;
pub mod ipc_contract;
mod metadata;
mod opened_audio;
pub mod output_artifact;
pub mod processing;
pub mod remote_source;
pub mod work_runtime;
// Re-export key public types needed by external integration tests without exposing full internal module structure
pub use metadata::{
    add_cover_art_stream_pre_header as ffmpeg_add_cover_art_stream_pre_header,
    set_container_metadata as ffmpeg_set_container_metadata,
    write_cover_art_packet_post_header as ffmpeg_write_cover_art_packet_post_header,
    CoverFormat as FfmpegCoverFormat,
};
pub use metadata::{
    extract_passthrough_metadata, finalize_artifact_metadata, read_audio_cover_thumbnail,
    read_metadata, save_metadata_intent, AlbumSortPatchOp, AudiobookMetadata,
    CoverArtPassthroughPolicy, MetadataIntentPatch, NamingMetadata, PassthroughSource, PatchOp,
};

pub mod audio;
pub use errors::{
    sanitize_path_for_display, sanitize_path_str_for_display, AppError, AppErrorCategory,
    AppErrorCode, AppErrorEnvelope,
};

use std::sync::Arc;
use tauri::{Emitter, LogicalSize, Manager, Size, WebviewWindow};

/// Type alias for managed JobRegistry state
pub type ManagedJobRegistry = Arc<processing::JobRegistry>;

const STARTUP_MAX_MONITOR_RATIO: f64 = 0.94;
const STARTUP_TARGET_ASPECT_RATIO: f64 = 16.0 / 10.0;
const STARTUP_PREFERRED_WIDTH: f64 = 1600.0;
const STARTUP_MIN_WIDTH: f64 = 1440.0;
const STARTUP_MIN_HEIGHT: f64 = 900.0;

fn monitor_fit_window_size(
    work_area_width: u32,
    work_area_height: u32,
    scale_factor: f64,
) -> Option<(f64, f64)> {
    if work_area_width == 0 || work_area_height == 0 {
        return None;
    }
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return None;
    }

    let logical_work_area_width = (work_area_width as f64) / scale_factor;
    let logical_work_area_height = (work_area_height as f64) / scale_factor;

    let max_width = (logical_work_area_width * STARTUP_MAX_MONITOR_RATIO).floor();
    let max_height = (logical_work_area_height * STARTUP_MAX_MONITOR_RATIO).floor();

    if max_width == 0.0 || max_height == 0.0 {
        return None;
    }

    let mut width = max_width.min(STARTUP_PREFERRED_WIDTH);
    let mut height = (width / STARTUP_TARGET_ASPECT_RATIO).round();

    if height > max_height {
        height = max_height;
        width = (height * STARTUP_TARGET_ASPECT_RATIO).round();
    }

    width = width.min(max_width);
    height = height.min(max_height);

    if width < STARTUP_MIN_WIDTH || height < STARTUP_MIN_HEIGHT {
        width = max_width;
        height = max_height;
    }

    Some((width.max(1.0), height.max(1.0)))
}

fn configure_startup_window(window: &WebviewWindow) -> Result<(), tauri::Error> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let monitor_size = monitor.size();
    let scale_factor = monitor.scale_factor();
    if let Some((width, height)) =
        monitor_fit_window_size(monitor_size.width, monitor_size.height, scale_factor)
    {
        window.set_size(Size::Logical(LogicalSize::new(width, height)))?;
        window.center()?;
        log::info!(
            "Startup window fit to monitor: monitor={}x{} @{}x, window={}x{} logical",
            monitor_size.width,
            monitor_size.height,
            scale_factor,
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

    // Initialize job registry with auto-detected concurrency (detected cores / 2)
    let job_registry: ManagedJobRegistry = Arc::new(processing::JobRegistry::auto());
    let work_runtime = work_runtime::WorkRuntime::default();
    log::info!(
        "Job registry initialized: max_concurrent = {}",
        job_registry.max_concurrent()
    );

    let specta_builder = ipc_contract::builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(job_registry)
        .manage(work_runtime)
        .manage(opened_audio::OpenedAudioFileQueue::default())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            let app_cache_dir = app.path().app_cache_dir().map_err(|error| {
                errors::AppError::General(format!("Failed to resolve app cache directory: {error}"))
            })?;
            audio::cleanup_abandoned_processing_workspaces(&app_cache_dir)?;
            let remote_runtime = remote_source::RemoteSourceRuntime::new(app.handle())?;
            remote_runtime.cleanup_abandoned_sessions()?;
            app.manage(remote_runtime);

            // Hydrate the durable user FFmpeg path into the audio toolchain
            // ingress so capability detection sees it from first use.
            match app
                .path()
                .app_config_dir()
                .map_err(|error| {
                    errors::AppError::General(format!(
                        "Failed to resolve app config directory: {error}"
                    ))
                })
                .and_then(|config_dir| app_settings::get_app_settings(&config_dir))
            {
                Ok(settings) => audio::set_user_external_ffmpeg_path(
                    settings.toolchain.external_ffmpeg_path.map(Into::into),
                ),
                Err(error) => log::warn!(
                    "Startup app settings hydration failed; using detected toolchain only: {error}"
                ),
            }

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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = event {
                let paths = opened_audio::collect_opened_audio_file_paths(urls);
                if paths.is_empty() {
                    return;
                }

                let Some(queue) = app.try_state::<opened_audio::OpenedAudioFileQueue>() else {
                    log::warn!("Opened audio queue state is unavailable");
                    return;
                };

                match queue.push_paths(paths) {
                    Ok(()) => {
                        let event = opened_audio::OpenedAudioFilesEvent::default();
                        if let Err(error) =
                            app.emit(opened_audio::OPENED_AUDIO_FILES_EVENT_NAME, event)
                        {
                            log::warn!("Failed to emit opened audio files event: {}", error);
                        }
                    }
                    Err(error) => {
                        log::warn!("Failed to queue opened audio files: {}", error);
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::monitor_fit_window_size;

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_returns_none_for_zero_inputs() {
        assert_eq!(monitor_fit_window_size(0, 1080, 1.0), None);
        assert_eq!(monitor_fit_window_size(1920, 0, 1.0), None);
        assert_eq!(monitor_fit_window_size(1920, 1080, 0.0), None);
    }

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_prefers_1600_by_1000_when_budget_allows() {
        let Some((width, height)) = monitor_fit_window_size(2560, 1600, 1.0) else {
            panic!("expected window size");
        };

        assert_eq!(width, 1600.0);
        assert_eq!(height, 1000.0);
    }

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_uses_logical_dimensions_for_high_dpi_monitors() {
        let Some((width, height)) = monitor_fit_window_size(3456, 2234, 2.0) else {
            panic!("expected window size");
        };

        assert_eq!(width, 1600.0);
        assert_eq!(height, 1000.0);
    }

    // EXCEPTION: tiny helper inline test.
    #[test]
    fn monitor_fit_window_size_uses_available_space_on_small_monitors() {
        let Some((width, height)) = monitor_fit_window_size(1024, 640, 1.0) else {
            panic!("expected window size");
        };

        assert_eq!(width, 962.0);
        assert_eq!(height, 601.0);
    }
}
