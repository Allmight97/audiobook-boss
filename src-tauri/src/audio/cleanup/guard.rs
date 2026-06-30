use crate::errors::{sanitize_path_for_display, Result};
use log::{debug, error};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// RAII guard for automatic cleanup of temporary directories and files
///
/// This guard ensures that temporary directories and files are cleaned up
/// when the guard is dropped, even if an error occurs or panic happens.
/// Multiple paths can be managed by a single guard.
pub struct CleanupGuard {
    paths: HashSet<PathBuf>,
    session_id: String,
}

impl CleanupGuard {
    /// Creates a new cleanup guard with the given session ID
    pub fn new(session_id: String) -> Self {
        debug!("Creating cleanup guard for session: {session_id}");
        Self {
            paths: HashSet::new(),
            session_id,
        }
    }

    // Internal accessor for sibling module operations
    pub(crate) fn raw_session_id(&self) -> &str {
        &self.session_id
    }

    /// Adds a path to be cleaned up when the guard is dropped
    pub fn add_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        debug!(
            "Session {}: Adding path to cleanup: {}",
            self.session_id,
            sanitize_path_for_display(&path_buf)
        );
        self.paths.insert(path_buf);
    }

    /// Removes a path from cleanup (useful if resource should be preserved)
    pub fn remove_path<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path_buf = path.as_ref().to_path_buf();
        let removed = self.paths.remove(&path_buf);
        if removed {
            debug!(
                "Session {}: Removed path from cleanup: {}",
                self.session_id,
                sanitize_path_for_display(&path_buf)
            );
        }
        removed
    }

    /// Performs immediate cleanup of all tracked paths
    pub fn cleanup_now(&mut self) -> Result<()> {
        debug!(
            "Session {}: Performing immediate cleanup of {} paths",
            self.session_id,
            self.paths.len()
        );

        let paths_to_clean: Vec<PathBuf> = self.paths.drain().collect();
        self.perform_cleanup(&paths_to_clean)
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.paths.is_empty() {
            debug!("Session {}: No paths to clean up", self.session_id);
            return;
        }

        debug!(
            "Session {}: Cleaning up {} paths on drop",
            self.session_id,
            self.paths.len()
        );

        let paths: Vec<PathBuf> = self.paths.iter().cloned().collect();

        if let Err(e) = self.perform_cleanup(&paths) {
            error!(
                "Session {}: Cleanup failed during drop: {}",
                self.session_id, e
            );
        }
    }
}
