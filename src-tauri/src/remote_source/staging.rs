use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

const REMOTE_SOURCE_DIR: &str = "remote-source";
const SESSIONS_DIR: &str = "sessions";
const ITEMS_DIR: &str = "items";

#[derive(Debug, Clone)]
pub(super) struct RemoteSourceStaging {
    root: PathBuf,
}

impl RemoteSourceStaging {
    pub(super) fn new(cache_dir: PathBuf) -> Self {
        Self {
            root: cache_dir.join(REMOTE_SOURCE_DIR),
        }
    }

    pub(super) fn session_root(&self) -> PathBuf {
        self.root.join(SESSIONS_DIR)
    }

    pub(super) fn create_job_dir(&self, job_id: &str) -> Result<PathBuf> {
        let path = self.session_root().join(job_id);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub(super) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        let session_root = self.session_root();
        if !session_root.exists() {
            return Ok(());
        }
        remove_owned_dir(&session_root)
    }

    pub(super) fn purge_session(&self, job_id: &str) -> Result<()> {
        let path = self.session_root().join(job_id);
        if !path.exists() {
            return Ok(());
        }
        ensure_owned_child(&self.session_root(), &path)?;
        remove_owned_dir(&path)
    }
}

pub(in crate::remote_source) fn create_item_dir(job_dir: &Path, item_id: &str) -> Result<PathBuf> {
    let path = job_dir.join(ITEMS_DIR).join(item_id);
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn ensure_owned_child(root: &Path, child: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .map_err(|error| AppError::ResourceCleanup(format!("Invalid staging root: {error}")))?;
    let child = child
        .canonicalize()
        .map_err(|error| AppError::ResourceCleanup(format!("Invalid staging child: {error}")))?;
    if child.starts_with(root) {
        return Ok(());
    }
    Err(AppError::ResourceCleanup(
        "Refusing to cleanup path outside ABB remote-source staging root".to_string(),
    ))
}

fn remove_owned_dir(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::ResourceCleanup(
            "Refusing to follow remote-source staging symlink during cleanup".to_string(),
        ));
    }
    fs::remove_dir_all(path)?;
    Ok(())
}
