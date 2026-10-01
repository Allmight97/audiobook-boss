// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

pub mod commands;
mod events;
mod intent_order;
pub mod ipc_contract;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, LogicalSize, Manager, Size, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

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

/// Builds the engine over this app's directories and identity, forwarding its
/// events to the webview.
fn start_engine(app: &tauri::App) -> abb_engine::Result<abb_engine::Engine> {
    let resolve = |kind: &str, path: tauri::Result<std::path::PathBuf>| {
        path.map_err(|error| {
            abb_engine::AppError::General(format!(
                "Failed to resolve app {kind} directory: {error}"
            ))
        })
    };
    abb_engine::Engine::start(abb_engine::EngineConfig {
        cache_dir: resolve("cache", app.path().app_cache_dir())?,
        config_dir: resolve("config", app.path().app_config_dir())?,
        app_identifier: app.config().identifier.clone(),
        events: std::sync::Arc::new(events::TauriEvents(app.handle().clone())),
        aaxclean_helper: None,
    })
}

/// How quitting is going: asked (and answered), engine shutting down, done.
#[derive(Default)]
struct Quit {
    confirmed: AtomicBool,
    shutting_down: AtomicBool,
    done: AtomicBool,
}

/// How long quitting waits for the engine to settle before exiting anyway.
const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

/// Shuts the engine down, then exits. Exports are cancelled, waiting saves
/// written, and every background task settled before the process ends.
fn shut_down_then_exit(app: &tauri::AppHandle) {
    let quit = app.state::<Quit>();
    if quit.shutting_down.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(engine) = app.try_state::<abb_engine::Engine>() {
            if tokio::time::timeout(SHUTDOWN_WAIT, engine.shutdown())
                .await
                .is_err()
            {
                log::warn!("Engine shutdown did not settle in time; exiting anyway");
            }
        }
        app.state::<Quit>().done.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Holds a quit until the engine has shut down, asking first when exports
/// are running. Returns whether the quit must be held.
fn hold_quit(app: &tauri::AppHandle) -> bool {
    let quit = app.state::<Quit>();
    if quit.done.load(Ordering::SeqCst) {
        return false;
    }
    if quit.shutting_down.load(Ordering::SeqCst) {
        return true;
    }
    let running = app
        .try_state::<abb_engine::Engine>()
        .map(|engine| engine.running_work())
        .unwrap_or_default();
    if running.exports == 0 || quit.confirmed.load(Ordering::SeqCst) {
        shut_down_then_exit(app);
        return true;
    }
    let exports = plural(running.exports, "export is", "exports are");
    let message = if running.waiting_writes == 0 {
        format!("{exports} still running. Quitting cancels them.")
    } else {
        format!(
            "{exports} still running, and metadata changes for {} wait for them. \
             Quitting cancels the exports and saves those changes first.",
            plural(running.waiting_writes, "file", "files")
        )
    };
    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Exports are still running")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit Anyway".to_string(),
            "Keep Open".to_string(),
        ))
        .show({
            let app = app.clone();
            move |quit| {
                if quit {
                    app.state::<Quit>().confirmed.store(true, Ordering::SeqCst);
                    shut_down_then_exit(&app);
                }
            }
        });
    true
}

/// Hands OS-opened files to the engine and tells the frontend to collect them.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
fn queue_opened_urls(app: &tauri::AppHandle, urls: Vec<tauri::Url>) {
    let paths = urls
        .into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .collect();
    let Some(engine) = app.try_state::<abb_engine::Engine>() else {
        log::warn!("Engine is unavailable; ignoring opened audio files");
        return;
    };
    match engine.queue_opened_audio_files(paths) {
        Ok(true) => {
            use tauri_specta::Event;
            let event = events::OpenedAudioFilesEvent::default();
            if let Err(error) = app.emit(events::OpenedAudioFilesEvent::NAME, event) {
                log::warn!("Failed to emit opened audio files event: {}", error);
            }
        }
        Ok(false) => {}
        Err(error) => log::warn!("Failed to queue opened audio files: {}", error),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with INFO level for production
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting AudioBook Boss application");

    let specta_builder = ipc_contract::builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            log::info!(
                "build_identity app_id={} app_version={} pid={} run_id={} {}",
                app.config().identifier,
                env!("CARGO_PKG_VERSION"),
                std::process::id(),
                std::env::var("ABB_RUN_ID").unwrap_or_else(|_| "unscoped".into()),
                abb_engine::ffmpeg_build_identity()
            );
            app.manage(start_engine(app)?);
            app.manage(Quit::default());
            app.manage(commands::FrontendLink::default());

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
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } if hold_quit(app) => {
                api.prevent_exit();
            }
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } if hold_quit(app) => {
                api.prevent_close();
            }
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            tauri::RunEvent::Opened { urls } => queue_opened_urls(app, urls),
            _ => {}
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
