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

/// Waits until no other writer holds `path`, then holds it.
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
/// lock.
fn lock_key(path: &Path) -> PathBuf {
    match (
        path.parent().and_then(|parent| parent.canonicalize().ok()),
        path.file_name(),
    ) {
        (Some(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

/// What ABB last saw of a file it wrote: its size and modification time.
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn a_second_writer_waits_for_the_first_to_let_go() {
        let folder = tempfile::TempDir::new().expect("folder");
        let path = folder.path().join("book.m4b");
        let first = lock_output_file(&path);
        let entered = Arc::new(AtomicBool::new(false));
        let second = std::thread::spawn({
            let entered = Arc::clone(&entered);
            // Another spelling of the same file shares the lock.
            let path = folder.path().join(".").join("book.m4b");
            move || {
                let _held = lock_output_file(&path);
                entered.store(true, Ordering::SeqCst);
            }
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(!entered.load(Ordering::SeqCst));
        drop(first);
        second.join().expect("second writer");
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn a_changed_file_has_a_different_identity() {
        let folder = tempfile::TempDir::new().expect("folder");
        let path = folder.path().join("book.m4b");
        std::fs::write(&path, b"written by ABB").expect("write");
        let written = FileIdentity::of(&path).expect("identity");
        assert_eq!(FileIdentity::of(&path).expect("identity"), written);
        std::fs::write(&path, b"replaced by something else").expect("replace");
        assert_ne!(FileIdentity::of(&path).expect("identity"), written);
    }
}
