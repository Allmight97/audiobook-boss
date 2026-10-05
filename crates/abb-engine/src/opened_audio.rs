//! Which files the operating system asked ABB to open can be imported.

use std::path::{Path, PathBuf};

use crate::audio::validate_input_audio_path;

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

#[cfg(test)]
#[path = "opened_audio_tests.rs"]
mod opened_audio_tests;
