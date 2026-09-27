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

    /// Startup policy: nothing staged by an earlier run is reused, so the whole
    /// sessions root goes.
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn staging(root: &TempDir) -> RemoteSourceStaging {
        RemoteSourceStaging::new(root.path().to_path_buf())
    }

    #[test]
    fn purge_refuses_a_session_outside_the_staging_root() {
        let cache = TempDir::new().expect("cache");
        let staging = staging(&cache);
        std::fs::create_dir_all(staging.session_root()).expect("session root");
        let outside = cache.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside dir");

        let error = staging
            .purge_session("../../outside")
            .expect_err("escaping job id refused");

        assert!(error.to_string().contains("outside"), "{error}");
        assert!(outside.exists(), "outside directory survives");
    }

    #[cfg(unix)]
    #[test]
    fn purge_refuses_a_symlinked_session() {
        let cache = TempDir::new().expect("cache");
        let staging = staging(&cache);
        let target = staging.session_root().join("target");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join("book.m4b"), b"audio").expect("staged file");
        std::os::unix::fs::symlink(&target, staging.session_root().join("job-link"))
            .expect("symlink");

        let error = staging
            .purge_session("job-link")
            .expect_err("symlink refused");

        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(target.join("book.m4b").exists(), "symlink target survives");
    }

    #[test]
    fn purge_removes_its_own_session() {
        let cache = TempDir::new().expect("cache");
        let staging = staging(&cache);
        let job_dir = staging.create_job_dir("job-1").expect("job dir");
        create_item_dir(&job_dir, "item-1").expect("item dir");

        staging.purge_session("job-1").expect("purge");

        assert!(!job_dir.exists());
        assert!(staging.session_root().exists());
    }

    #[test]
    fn startup_removes_every_abandoned_session() {
        let cache = TempDir::new().expect("cache");
        let staging = staging(&cache);
        staging.create_job_dir("job-1").expect("job one");
        staging.create_job_dir("job-2").expect("job two");

        staging
            .cleanup_abandoned_sessions()
            .expect("startup cleanup");

        assert!(!staging.session_root().exists());
        assert!(cache.path().join(REMOTE_SOURCE_DIR).exists());
    }

    #[cfg(unix)]
    #[test]
    fn startup_refuses_a_symlinked_sessions_root() {
        let cache = TempDir::new().expect("cache");
        let staging = staging(&cache);
        let target = cache.path().join("elsewhere");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::create_dir_all(cache.path().join(REMOTE_SOURCE_DIR)).expect("remote dir");
        std::os::unix::fs::symlink(&target, staging.session_root()).expect("symlink");

        let error = staging
            .cleanup_abandoned_sessions()
            .expect_err("symlinked root refused");

        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(target.exists());
    }
}
