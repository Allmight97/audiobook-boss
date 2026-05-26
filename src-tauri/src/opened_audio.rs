//! Runtime queue for audio files opened by the operating system.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::audio::validate_input_audio_path;
use crate::errors::{AppError, Result};

pub const OPENED_AUDIO_FILES_EVENT_NAME: &str = "opened-audio-files";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenedAudioFilesEvent {
    pub paths: Vec<String>,
}

impl tauri_specta::Event for OpenedAudioFilesEvent {
    const NAME: &'static str = OPENED_AUDIO_FILES_EVENT_NAME;
}

#[derive(Default)]
pub struct OpenedAudioFileQueue {
    paths: Mutex<Vec<PathBuf>>,
}

impl OpenedAudioFileQueue {
    pub fn push_paths(&self, paths: Vec<PathBuf>) -> Result<Vec<String>> {
        let mut guard = self
            .paths
            .lock()
            .map_err(|_| AppError::General("Opened audio queue lock was poisoned".to_string()))?;
        let mut seen = guard.iter().cloned().collect::<HashSet<_>>();

        for path in paths {
            if seen.insert(path.clone()) {
                guard.push(path);
            }
        }

        Ok(paths_to_strings(&guard))
    }

    pub fn take_paths(&self) -> Result<Vec<String>> {
        let mut guard = self
            .paths
            .lock()
            .map_err(|_| AppError::General("Opened audio queue lock was poisoned".to_string()))?;
        let paths = paths_to_strings(&guard);
        guard.clear();
        Ok(paths)
    }
}

pub fn collect_opened_audio_file_paths(urls: Vec<tauri::Url>) -> Vec<PathBuf> {
    urls.into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .filter_map(|path| validate_opened_audio_path(&path))
        .collect()
}

fn validate_opened_audio_path(path: &Path) -> Option<PathBuf> {
    match validate_input_audio_path(path) {
        Ok(path) => Some(path),
        Err(error) => {
            log::warn!("Ignoring OS-opened audio path: {}", error);
            None
        }
    }
}

fn paths_to_strings(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn opened_audio_file_queue_drains_once_and_dedupes() {
        let queue = OpenedAudioFileQueue::default();
        let first = PathBuf::from("/tmp/book-1.m4b");
        let second = PathBuf::from("/tmp/book-2.m4b");

        queue
            .push_paths(vec![first.clone(), second.clone(), first.clone()])
            .expect("push");

        assert_eq!(
            queue.take_paths().expect("first drain"),
            vec![
                first.to_string_lossy().to_string(),
                second.to_string_lossy().to_string(),
            ]
        );
        assert!(queue.take_paths().expect("second drain").is_empty());
    }

    #[test]
    fn collect_opened_audio_file_paths_keeps_supported_file_urls() {
        let temp = TempDir::new().expect("temp dir");
        let supported = temp.path().join("Book.m4b");
        let unsupported = temp.path().join("cover.jpg");
        std::fs::write(&supported, b"audio").expect("supported file");
        std::fs::write(&unsupported, b"image").expect("unsupported file");

        let supported_url = tauri::Url::from_file_path(&supported).expect("supported file url");
        let unsupported_url =
            tauri::Url::from_file_path(&unsupported).expect("unsupported file url");
        let web_url = tauri::Url::parse("https://example.com/Book.m4b").expect("web url");

        let actual = collect_opened_audio_file_paths(vec![supported_url, unsupported_url, web_url]);

        assert_eq!(actual, vec![supported.canonicalize().expect("canonical")]);
    }
}
