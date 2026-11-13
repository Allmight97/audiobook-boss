use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};
use crate::owned_dir::OwnedRoot;

const REMOTE_SOURCE_DIR: &str = "remote-source";
const SESSIONS_DIR: &str = "sessions";
const ITEMS_DIR: &str = "items";
const STAGING_LABEL: &str = "remote-source staging";
const SYMLINKED_SESSIONS: &str = "ABB's download staging folder is a symlink, so ABB will not download into it. Remove the link and try again.";

#[derive(Debug, Clone)]
pub(super) struct RemoteSourceStaging {
    session_root: PathBuf,
}

impl RemoteSourceStaging {
    /// Keeps the staging folder's canonical spelling. The sessions folder
    /// below it is created by ABB, so job folders are canonical too unless it
    /// is a symlink, which `create_job_dir` refuses.
    pub(super) fn new(cache_dir: &Path) -> Result<Self> {
        let root = cache_dir.join(REMOTE_SOURCE_DIR);
        fs::create_dir_all(&root)?;
        Ok(Self {
            session_root: root.canonicalize()?.join(SESSIONS_DIR),
        })
    }

    pub(super) fn session_root(&self) -> PathBuf {
        self.session_root.clone()
    }

    pub(super) fn create_job_dir(&self, job_id: &str) -> Result<PathBuf> {
        fs::create_dir_all(&self.session_root)?;
        if fs::symlink_metadata(&self.session_root)?
            .file_type()
            .is_symlink()
        {
            return Err(AppError::FileValidation(SYMLINKED_SESSIONS.to_string()));
        }
        let path = self.session_root.join(job_id);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub(super) fn cleanup_abandoned_sessions(&self) -> Result<()> {
        self.with_sessions_root(|root| root.remove_root())
    }

    pub(super) fn purge_session(&self, job_id: &str) -> Result<()> {
        self.with_sessions_root(|root| root.remove_child(&root.path.join(job_id)))
    }

    fn with_sessions_root(&self, remove: impl FnOnce(OwnedRoot<'_>) -> Result<()>) -> Result<()> {
        let session_root = self.session_root();
        remove(OwnedRoot {
            path: &session_root,
            label: STAGING_LABEL,
        })
    }
}

pub(in crate::remote_source) fn create_item_dir(job_dir: &Path, item_id: &str) -> Result<PathBuf> {
    let path = job_dir.join(ITEMS_DIR).join(item_id);
    fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
