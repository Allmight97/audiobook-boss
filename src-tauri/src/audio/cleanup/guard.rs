use crate::errors::Result;
#[cfg(any(test, feature = "safe-ffmpeg"))]
use crate::errors::AppError;
use log::{debug, error};
#[cfg(any(test, feature = "safe-ffmpeg"))]
use log::warn;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(any(test, feature = "safe-ffmpeg"))]
use std::process::Child;
#[cfg(any(test, feature = "safe-ffmpeg"))]
use std::sync::{Arc, Mutex};

/// RAII guard for automatic cleanup of temporary directories and files
///
/// This guard ensures that temporary directories and files are cleaned up
/// when the guard is dropped, even if an error occurs or panic happens.
/// Multiple paths can be managed by a single guard.
pub struct CleanupGuard {
    paths: HashSet<PathBuf>,
    session_id: String,
    enabled: bool,
}

impl CleanupGuard {
    /// Creates a new cleanup guard with the given session ID
    pub fn new(session_id: String) -> Self {
        debug!("Creating cleanup guard for session: {session_id}");
        Self {
            paths: HashSet::new(),
            session_id,
            enabled: true,
        }
    }

    // Internal accessor for sibling module operations
    pub(crate) fn raw_session_id(&self) -> &str {
        &self.session_id
    }

    /// Adds a path to be cleaned up when the guard is dropped
    #[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(dead_code))]
    pub fn add_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        debug!(
            "Session {}: Adding path to cleanup: {}",
            self.session_id,
            path_buf.display()
        );
        self.paths.insert(path_buf);
    }

    /// Adds multiple paths to be cleaned up
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn add_paths<I, P>(&mut self, paths: I)
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        for path in paths {
            self.add_path(path);
        }
    }

    /// Removes a path from cleanup (useful if resource should be preserved)
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn remove_path<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path_buf = path.as_ref().to_path_buf();
        let removed = self.paths.remove(&path_buf);
        if removed {
            debug!(
                "Session {}: Removed path from cleanup: {}",
                self.session_id,
                path_buf.display()
            );
        }
        removed
    }

    /// Disables cleanup for debugging purposes
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn disable_cleanup(&mut self) {
        debug!("Session {}: Cleanup disabled for debugging", self.session_id);
        self.enabled = false;
    }

    /// Enables cleanup (default state)
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn enable_cleanup(&mut self) {
        debug!("Session {}: Cleanup enabled", self.session_id);
        self.enabled = true;
    }

    /// Returns the number of paths being tracked
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Returns the session ID
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Performs immediate cleanup of all tracked paths
    pub fn cleanup_now(&mut self) -> Result<()> {
        if !self.enabled {
            debug!(
                "Session {}: Cleanup disabled, skipping immediate cleanup",
                self.session_id
            );
            return Ok(());
        }

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
        if !self.enabled {
            debug!(
                "Session {}: Cleanup disabled, skipping drop cleanup",
                self.session_id
            );
            return;
        }

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
                self.session_id,
                e
            );
        }
    }
}

/// RAII guard for automatic process termination
#[cfg(any(test, feature = "safe-ffmpeg"))]
pub struct ProcessGuard {
    process: Arc<Mutex<Option<Child>>>,
    session_id: String,
    description: String,
    enabled: bool,
}

#[cfg(any(test, feature = "safe-ffmpeg"))]
impl ProcessGuard {
    /// Creates a new process guard for the given child process
    pub fn new(process: Child, session_id: String, description: String) -> Self {
        debug!("Session {session_id}: Creating process guard for: {description}");
        Self {
            process: Arc::new(Mutex::new(Some(process))),
            session_id,
            description,
            enabled: true,
        }
    }

    /// Gets a clone of the process Arc for sharing across threads
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn process_handle(&self) -> Arc<Mutex<Option<Child>>> {
        Arc::clone(&self.process)
    }

    /// Waits for the process to complete and returns the exit status
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn wait(self) -> Result<std::process::ExitStatus> {
        debug!(
            "Session {}: Waiting for process completion: {}",
            self.session_id, self.description
        );

        let mut process_lock = self
            .process
            .lock()
            .map_err(|_| AppError::General("Failed to acquire process lock".to_string()))?;

        match process_lock.take() {
            Some(mut child) => {
                let status = child.wait().map_err(AppError::Io)?;
                debug!(
                    "Session {}: Process completed with status: {:?}",
                    self.session_id, status
                );
                Ok(status)
            }
            None => Err(AppError::General("Process already consumed".to_string())),
        }
    }

    /// Attempts to terminate the process gracefully, then forcefully if needed
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn terminate(&self) -> Result<()> {
        if !self.enabled {
            debug!("Session {}: Process termination disabled", self.session_id);
            return Ok(());
        }

        let mut process_lock = self
            .process
            .lock()
            .map_err(|_| AppError::General("Failed to acquire process lock".to_string()))?;

        match process_lock.as_mut() {
            Some(child) => {
                debug!(
                    "Session {}: Terminating process: {}",
                    self.session_id, self.description
                );

                if let Err(e) = child.kill() {
                    warn!(
                        "Session {}: Failed to kill process {}: {}",
                        self.session_id, self.description, e
                    );
                    return Err(AppError::General(format!(
                        "Process termination failed: {e}"
                    )));
                }

                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!(
                            "Session {}: Process terminated with status: {:?}",
                            self.session_id, status
                        );
                    }
                    Ok(None) => {
                        debug!("Session {}: Process termination initiated", self.session_id);
                    }
                    Err(e) => {
                        warn!(
                            "Session {}: Error checking process status: {}",
                            self.session_id, e
                        );
                    }
                }

                *process_lock = None;
                Ok(())
            }
            None => {
                debug!(
                    "Session {}: Process already terminated or consumed",
                    self.session_id
                );
                Ok(())
            }
        }
    }

    /// Disables automatic termination for debugging
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn disable_termination(&mut self) {
        debug!(
            "Session {}: Process termination disabled for debugging",
            self.session_id
        );
        self.enabled = false;
    }

    /// Enables automatic termination (default state)
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn enable_termination(&mut self) {
        debug!("Session {}: Process termination enabled", self.session_id);
        self.enabled = true;
    }

    /// Returns the session ID
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the process description
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(any(test, feature = "safe-ffmpeg"))]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if !self.enabled {
            debug!(
                "Session {}: Process termination disabled, skipping drop cleanup",
                self.session_id
            );
            return;
        }

        debug!(
            "Session {}: Terminating process on drop: {}",
            self.session_id, self.description
        );

        if let Err(e) = self.terminate() {
            error!(
                "Session {}: Process termination failed during drop: {}",
                self.session_id, e
            );
        }
    }
}

// Integration utilities for use with ProcessingContext
impl CleanupGuard {
    #[cfg(any(test, feature = "safe-ffmpeg"))]
    pub fn from_context(context: &crate::audio::ProcessingContext) -> Self {
        Self::new(context.session.id())
    }
}

#[cfg(any(test, feature = "safe-ffmpeg"))]
impl ProcessGuard {
    pub fn from_context(
        process: Child,
        context: &crate::audio::ProcessingContext,
        description: String,
    ) -> Self {
        Self::new(process, context.session.id(), description)
    }
}


