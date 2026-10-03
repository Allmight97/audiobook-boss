//! Runtime queue for audio files opened by the operating system.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::audio::validate_input_audio_path;
use crate::errors::{AppError, Result};

#[derive(Default)]
pub(crate) struct OpenedAudioFileQueue {
    paths: Mutex<Vec<PathBuf>>,
}

impl OpenedAudioFileQueue {
    pub(crate) fn push_paths(&self, paths: Vec<PathBuf>) -> Result<()> {
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

        Ok(())
    }

    pub(crate) fn take_paths(&self) -> Result<Vec<String>> {
        let mut guard = self
            .paths
            .lock()
            .map_err(|_| AppError::General("Opened audio queue lock was poisoned".to_string()))?;
        let paths = paths_to_strings(&guard);
        guard.clear();
        Ok(paths)
    }
}

/// Keeps the paths that are supported local audio files.
pub(crate) fn supported_opened_audio_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
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
#[path = "opened_audio_tests.rs"]
mod opened_audio_tests;
