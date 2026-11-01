//! Output files after acceptance: one writer at a time per file across every
//! export in the process, and the identity that tells a later tag save the
//! file is still the one ABB wrote.
//!
//! Publication and every tag save on a published output take the lock, so an
//! export that replaces a file can never be overwritten by another export's
//! tag save that read the old file first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, PoisonError};
use std::time::SystemTime;

use crate::errors::{AppError, Result};

static WRITERS: LazyLock<(Mutex<HashSet<PathBuf>>, Condvar)> =
    LazyLock::new(|| (Mutex::new(HashSet::new()), Condvar::new()));

/// Held while one writer owns an output file. Blocking: take it on a blocking
/// thread.
pub(crate) struct OutputFileLock {
    key: PathBuf,
}

pub(crate) fn lock_output_file(path: &Path) -> OutputFileLock {
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

/// One spelling per file: the canonical folder plus the file name, so two
/// exports naming the same file through different folder spellings share a
/// lock. macOS and Windows disks usually ignore case, so there the key does
/// too; two files differing only in case then merely wait for each other.
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
pub(crate) struct FileIdentity {
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
