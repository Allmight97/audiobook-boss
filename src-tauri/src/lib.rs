pub mod commands;
mod cover_protocol;
mod events;
mod intent_order;
pub mod ipc_contract;
mod log_file;
mod opened_files;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{LogicalSize, Manager, Size, WebviewWindow};
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
    })
}

fn quit_as_second_copy(app: &tauri::App, message: String) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            log::warn!("The second ABB window stayed visible: {error}");
        }
    }
    let handle = app.handle().clone();
    app.dialog()
        .message(message)
        .title("ABB is already running")
        .show(move |_| handle.exit(0));
}

#[derive(Default)]
struct Quit {
    shutting_down: AtomicBool,
    done: AtomicBool,
}

const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug)]
enum ShutdownWait<T> {
    Finished(T),
    QuitChosen,
}

/// Awaits `shutdown`, asking after each `SHUTDOWN_WAIT` whether to keep
/// waiting.
async fn settle_or_ask<T, A>(
    shutdown: impl std::future::Future<Output = T>,
    mut keep_waiting: impl FnMut() -> A,
) -> ShutdownWait<T>
where
    A: std::future::Future<Output = bool>,
{
    tokio::pin!(shutdown);
    loop {
        if let Ok(outcome) = tokio::time::timeout(SHUTDOWN_WAIT, &mut shutdown).await {
            return ShutdownWait::Finished(outcome);
        }
        if !keep_waiting().await {
            return ShutdownWait::QuitChosen;
        }
    }
}

async fn ask_to_retry_settings_save(app: tauri::AppHandle, reason: String) -> bool {
    let (answer, answered) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(format!(
            "ABB could not save your settings: {reason} Quitting now loses the changes \
             made since they were last saved."
        ))
        .title("Settings not saved")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Retry".to_string(),
            "Quit Anyway".to_string(),
        ))
        .show(move |retry| {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the quit flow has stopped waiting if the receiver is gone"
            )]
            let _ = answer.send(retry);
        });
    answered.await.unwrap_or(false)
}

async fn ask_to_keep_waiting(app: tauri::AppHandle) -> bool {
    let (answer, answered) = tokio::sync::oneshot::channel();
    app.dialog()
        .message(
            "ABB is still finishing: metadata changes are being written and running work \
             is stopping. Quitting now can leave those changes unsaved.",
        )
        .title("Still finishing")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Keep Waiting".to_string(),
            "Quit Now".to_string(),
        ))
        .show(move |wait| {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the quit flow has stopped waiting if the receiver is gone"
            )]
            let _ = answer.send(wait);
        });
    answered.await.unwrap_or(false)
}

async fn quit_when_saved<S, W, R>(
    mut shutdown: impl FnMut() -> S,
    mut keep_waiting: impl FnMut() -> W,
    mut retry_save: impl FnMut(String) -> R,
) where
    S: std::future::Future<Output = abb_engine::ShutdownOutcome>,
    W: std::future::Future<Output = bool>,
    R: std::future::Future<Output = bool>,
{
    loop {
        match settle_or_ask(shutdown(), &mut keep_waiting).await {
            ShutdownWait::Finished(abb_engine::ShutdownOutcome::Settled) => break,
            ShutdownWait::Finished(abb_engine::ShutdownOutcome::SettingsUnsaved { error }) => {
                if !retry_save(error.message).await {
                    log::warn!("Quit with unsaved settings, at the user's choice");
                    break;
                }
            }
            ShutdownWait::QuitChosen => {
                log::warn!("Quit before the engine settled, at the user's choice");
                break;
            }
        }
    }
}

fn shut_down_then_exit(app: &tauri::AppHandle) {
    let quit = app.state::<Quit>();
    if quit.shutting_down.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(engine) = app.try_state::<abb_engine::Engine>() {
            let engine = engine.inner().clone();
            quit_when_saved(
                || engine.shutdown(),
                || ask_to_keep_waiting(app.clone()),
                |reason| ask_to_retry_settings_save(app.clone(), reason),
            )
            .await;
        }
        app.state::<Quit>().done.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

#[derive(Debug, PartialEq)]
struct QuitPrompt {
    title: &'static str,
    message: String,
}

fn quit_prompt(running: &abb_engine::RunningWork) -> QuitPrompt {
    let waiting = (running.exports == 0 && running.waiting_writes > 0).then(|| {
        format!(
            "Metadata changes for {} wait to be saved. Quitting saves them first.",
            plural(running.waiting_writes, "file", "files")
        )
    });
    let exports = (running.exports > 0).then(|| {
        let exports = plural(running.exports, "export is", "exports are");
        if running.waiting_writes == 0 {
            format!("{exports} still running. Quitting cancels them.")
        } else {
            format!(
                "{exports} still running, and metadata changes for {} wait for them. \
                 Quitting cancels the exports and saves those changes first.",
                plural(running.waiting_writes, "file", "files")
            )
        }
    });
    let downloads = (running.acquisitions > 0).then(|| {
        format!(
            "{} still running. Quitting stops {} and discards what was downloaded.",
            plural(
                running.acquisitions,
                "Audible download is",
                "Audible downloads are"
            ),
            if running.acquisitions == 1 {
                "it"
            } else {
                "them"
            }
        )
    });
    match (exports.or(waiting), downloads) {
        (Some(work), None) if running.exports > 0 => QuitPrompt {
            title: "Exports are still running",
            message: work,
        },
        (Some(work), None) => QuitPrompt {
            title: "Saves are still waiting",
            message: work,
        },
        (Some(work), Some(downloads)) => QuitPrompt {
            title: "Work is still running",
            message: format!("{work} {downloads}"),
        },
        (None, downloads) => QuitPrompt {
            title: "Downloads are still running",
            message: downloads.unwrap_or_default(),
        },
    }
}

#[derive(Debug, PartialEq)]
enum QuitStep {
    ShutDown,
    Ask(abb_engine::RunningWork),
}

fn next_quit_step(
    close_for_quit: impl FnOnce(&abb_engine::QuitConsent) -> Result<(), abb_engine::RunningWork>,
    consent: &abb_engine::QuitConsent,
) -> QuitStep {
    match close_for_quit(consent) {
        Ok(()) => QuitStep::ShutDown,
        Err(running) => QuitStep::Ask(running),
    }
}

fn hold_quit(app: &tauri::AppHandle) -> bool {
    let quit = app.state::<Quit>();
    if quit.done.load(Ordering::SeqCst) {
        return false;
    }
    if !quit.shutting_down.load(Ordering::SeqCst) {
        quit_with_consent(app, &abb_engine::QuitConsent::default());
    }
    true
}

/// Quits when the engine closes for the work `consent` covers; otherwise
/// asks about the work quitting would stop now, and asks again if that
/// changes before the user answers.
fn quit_with_consent(app: &tauri::AppHandle, consent: &abb_engine::QuitConsent) {
    let step = next_quit_step(
        |consent| {
            app.try_state::<abb_engine::Engine>()
                .map_or(Ok(()), |engine| engine.close_for_quit(consent))
        },
        consent,
    );
    let QuitStep::Ask(running) = step else {
        return shut_down_then_exit(app);
    };
    let prompt = quit_prompt(&running);
    let consent = running.consent;
    let app = app.clone();
    app.dialog()
        .message(prompt.message)
        .title(prompt.title)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit Anyway".to_string(),
            "Keep Open".to_string(),
        ))
        .show({
            let app = app.clone();
            move |quit| {
                if quit {
                    quit_with_consent(&app, &consent);
                }
            }
        });
}

pub fn run() {
    log_file::init_logger();

    log::info!("Starting AudioBook Boss application");

    let specta_builder = ipc_contract::builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(opened_files::HeldFiles::default())
        .invoke_handler(specta_builder.invoke_handler())
        .register_asynchronous_uri_scheme_protocol(cover_protocol::SCHEME, cover_protocol::handle)
        .setup(move |app| {
            specta_builder.mount_events(app);
            log::info!(
                "build_identity app_id={} app_version={} pid={} run_id={} {}",
                app.config().identifier,
                env!("CARGO_PKG_VERSION"),
                std::process::id(),
                std::env::var("ABB_RUN_ID").unwrap_or_else(|_| "unscoped".into()),
                abb_engine::media_build_identity()
            );
            app.manage(Quit::default());
            app.manage(commands::FrontendLink::default());
            match start_engine(app) {
                Ok(engine) => {
                    opened_files::import_held(app.handle(), &engine);
                    app.manage(engine)
                }
                Err(error @ abb_engine::AppError::AlreadyRunning(_)) => {
                    quit_as_second_copy(app, error.to_string());
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            };

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
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Opened { urls } => opened_files::open(app, urls),
            _ => {}
        });
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "lib_quit_tests.rs"]
mod quit_tests;
