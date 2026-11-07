use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use abb_engine::Engine;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Default)]
pub(crate) struct HeldFiles(Mutex<Vec<PathBuf>>);

impl HeldFiles {
    fn paths(&self) -> MutexGuard<'_, Vec<PathBuf>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn open<R: Runtime>(app: &AppHandle<R>, urls: Vec<tauri::Url>) {
    let paths: Vec<PathBuf> = urls
        .into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .collect();
    match app.try_state::<Engine>() {
        Some(engine) => import(&engine, paths),
        None => {
            log::info!("Holding {} opened files for the engine", paths.len());
            app.state::<HeldFiles>().paths().extend(paths);
        }
    }
}

pub(crate) fn import_held<R: Runtime>(app: &AppHandle<R>, engine: &Engine) {
    let paths = std::mem::take(&mut *app.state::<HeldFiles>().paths());
    if !paths.is_empty() {
        import(engine, paths);
    }
}

fn import(engine: &Engine, paths: Vec<PathBuf>) {
    if let Err(error) = tauri::async_runtime::block_on(engine.open_audio_files(paths)) {
        log::warn!("Opened audio files were not imported: {error}");
    }
}

#[cfg(test)]
#[path = "opened_files_tests.rs"]
mod tests;
