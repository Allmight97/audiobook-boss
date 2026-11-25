use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, PoisonError};
use std::time::SystemTime;

use crate::errors::{AppError, Result};

static WRITERS: LazyLock<(Mutex<HashSet<PathBuf>>, Condvar)> =
    LazyLock::new(|| (Mutex::new(HashSet::new()), Condvar::new()));

pub struct OutputFileLock {
    key: PathBuf,
}

pub fn lock_output_file(path: &Path) -> OutputFileLock {
    let key = lock_key(path);
    let (writers, freed) = &*WRITERS;
    let mut held = writers.lock().unwrap_or_else(PoisonError::into_inner);
    while held.contains(&key) {
        held = freed.wait(held).unwrap_or_else(PoisonError::into_inner);
    }
    held.insert(key.clone());
    OutputFileLock { key }
}

impl Drop for OutputFileLock {
    fn drop(&mut self) {
        let (writers, freed) = &*WRITERS;
        writers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
        freed.notify_all();
    }
}

fn lock_key(path: &Path) -> PathBuf {
    let key = match (
        path.parent().and_then(|parent| parent.canonicalize().ok()),
        path.file_name(),
    ) {
        (Some(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    };
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        PathBuf::from(key.to_string_lossy().to_lowercase())
    } else {
        key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileIdentity {
    pub(crate) fn of(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path).map_err(AppError::Io)?;
        Ok(Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
