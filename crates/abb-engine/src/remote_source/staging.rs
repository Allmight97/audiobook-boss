use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::Result;
use crate::owned_dir::OwnedRoot;

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
        self.with_sessions_root(|root| root.remove_root())
    }

    pub(super) fn purge_session(&self, job_id: &str) -> Result<()> {
        self.with_sessions_root(|root| root.remove_child(&root.path.join(job_id)))
    }

    fn with_sessions_root(&self, remove: impl FnOnce(OwnedRoot<'_>) -> Result<()>) -> Result<()> {
        let session_root = self.session_root();
        remove(OwnedRoot {
            path: &session_root,
            label: "remote-source staging",
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
