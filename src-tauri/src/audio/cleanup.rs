//! RAII cleanup guards for automatic resource management
//! 
//! This module provides guards that ensure proper cleanup of temporary resources
//! even if processing fails or panics. The guards implement RAII patterns for
//! automatic cleanup when they go out of scope.

use crate::errors::{AppError, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use log::{debug, warn, error};

/// RAII guard for automatic cleanup of temporary directories and files
/// 
/// This guard ensures that temporary directories and files are cleaned up
/// when the guard is dropped, even if an error occurs or panic happens.
/// Multiple paths can be managed by a single guard.
pub struct CleanupGuard {
    /// Set of paths to clean up on drop
    paths: HashSet<PathBuf>,
    /// Session ID for tracking and debugging
    session_id: String,
    /// Whether cleanup should be performed (can be disabled for debugging)
    enabled: bool,
}

impl CleanupGuard {
    /// Creates a new cleanup guard with the given session ID
    /// 
    /// # Arguments
    /// * `session_id` - Unique identifier for tracking cleanup operations
    /// 
    /// # Example
    /// ```rust,no_run
    /// # use std::collections::HashSet;
    /// # struct CleanupGuard {
    /// #     paths: HashSet<std::path::PathBuf>,
    /// #     session_id: String,
    /// #     enabled: bool,
    /// # }
    /// # impl CleanupGuard {
    /// #     fn new(session_id: String) -> Self {
    /// #         Self {
    /// #             paths: HashSet::new(),
    /// #             session_id,
    /// #             enabled: true,
    /// #         }
    /// #     }
    /// # }
    /// let guard = CleanupGuard::new("session-123".to_string());
    /// ```
    pub fn new(session_id: String) -> Self {
        debug!("Creating cleanup guard for session: {session_id}");
        Self {
            paths: HashSet::new(),
            session_id,
            enabled: true,
        }
    }
    
    /// Adds a path to be cleaned up when the guard is dropped
    /// 
    /// # Arguments
    /// * `path` - Path to directory or file to be cleaned up
    /// 
    /// # Example
    /// ```rust,no_run
    /// # use std::collections::HashSet;
    /// # use std::path::{Path, PathBuf};
    /// # struct CleanupGuard {
    /// #     paths: HashSet<PathBuf>,
    /// #     session_id: String,
    /// #     enabled: bool,
    /// # }
    /// # impl CleanupGuard {
    /// #     fn new(session_id: String) -> Self {
    /// #         Self {
    /// #             paths: HashSet::new(),
    /// #             session_id,
    /// #             enabled: true,
    /// #         }
    /// #     }
    /// #     fn add_path<P: AsRef<Path>>(&mut self, path: P) {
    /// #         self.paths.insert(path.as_ref().to_path_buf());
    /// #     }
    /// # }
    /// let mut guard = CleanupGuard::new("session-123".to_string());
    /// guard.add_path("/tmp/audiobook_processing");
    /// ```
    pub fn add_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        debug!("Session {}: Adding path to cleanup: {}", 
               self.session_id, path_buf.display());
        self.paths.insert(path_buf);
    }
    
    /// Adds multiple paths to be cleaned up
    /// 
    /// # Arguments
    /// * `paths` - Iterator of paths to be cleaned up
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
    /// 
    /// # Arguments
    /// * `path` - Path to remove from cleanup list
    /// 
    /// # Returns
    /// `true` if the path was removed, `false` if it wasn't in the list
    pub fn remove_path<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path_buf = path.as_ref().to_path_buf();
        let removed = self.paths.remove(&path_buf);
        if removed {
            debug!("Session {}: Removed path from cleanup: {}", 
                   self.session_id, path_buf.display());
        }
        removed
    }
    
    /// Disables cleanup for debugging purposes
    /// 
    /// When disabled, paths will not be cleaned up on drop.
    /// This is useful for debugging to inspect temporary files.
    pub fn disable_cleanup(&mut self) {
        debug!("Session {}: Cleanup disabled for debugging", self.session_id);
        self.enabled = false;
    }
    
    /// Enables cleanup (default state)
    pub fn enable_cleanup(&mut self) {
        debug!("Session {}: Cleanup enabled", self.session_id);
        self.enabled = true;
    }
    
    /// Returns the number of paths being tracked
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }
    
    /// Returns the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    /// Performs immediate cleanup of all tracked paths
    /// 
    /// This method can be called manually to clean up resources before
    /// the guard is dropped. After calling this, the paths list is cleared.
    /// 
    /// # Returns
    /// `Ok(())` if all cleanups succeeded, or the first error encountered
    pub fn cleanup_now(&mut self) -> Result<()> {
        if !self.enabled {
            debug!("Session {}: Cleanup disabled, skipping immediate cleanup", 
                   self.session_id);
            return Ok(());
        }
        
        debug!("Session {}: Performing immediate cleanup of {} paths", 
               self.session_id, self.paths.len());
        
        let paths_to_clean: Vec<PathBuf> = self.paths.drain().collect();
        self.perform_cleanup(&paths_to_clean)
    }
    
    /// Internal cleanup implementation that never panics
    fn perform_cleanup(&self, paths: &[PathBuf]) -> Result<()> {
        let mut first_error: Option<AppError> = None;
        
        for path in paths {
            if let Err(e) = self.cleanup_single_path(path) {
                error!("Session {}: Failed to cleanup {}: {}", 
                       self.session_id, path.display(), e);
                
                // Store first error but continue cleaning other paths
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        
        // Return first error if any occurred
        match first_error {
            Some(err) => Err(err),
            None => {
                debug!("Session {}: All cleanup operations completed successfully", 
                       self.session_id);
                Ok(())
            }
        }
    }
    
    /// Clean up a single path (file or directory)
    fn cleanup_single_path(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!("Session {}: Path already removed: {}", 
                   self.session_id, path.display());
            return Ok(());
        }
        
        if path.is_dir() {
            debug!("Session {}: Removing directory: {}", 
                   self.session_id, path.display());
            std::fs::remove_dir_all(path)
                .map_err(AppError::Io)?;
        } else {
            debug!("Session {}: Removing file: {}", 
                   self.session_id, path.display());
            std::fs::remove_file(path)
                .map_err(AppError::Io)?;
        }
        
        Ok(())
    }
}

impl Drop for CleanupGuard {
    /// Automatically clean up all tracked paths when guard is dropped
    /// 
    /// This method never panics, even if cleanup fails. Errors are logged
    /// but not propagated to avoid issues during stack unwinding.
    fn drop(&mut self) {
        if !self.enabled {
            debug!("Session {}: Cleanup disabled, skipping drop cleanup", 
                   self.session_id);
            return;
        }
        
        if self.paths.is_empty() {
            debug!("Session {}: No paths to clean up", self.session_id);
            return;
        }
        
        debug!("Session {}: Cleaning up {} paths on drop", 
               self.session_id, self.paths.len());
        
        let paths: Vec<PathBuf> = self.paths.iter().cloned().collect();
        
        // Never panic in Drop - just log errors
        if let Err(e) = self.perform_cleanup(&paths) {
            error!("Session {}: Cleanup failed during drop: {}", 
                   self.session_id, e);
        }
    }
}

/// RAII guard for automatic process termination
/// 
/// This guard wraps a child process and ensures it's properly terminated
/// when the guard is dropped, even if an error occurs or panic happens.
pub struct ProcessGuard {
    /// The child process being managed
    process: Arc<Mutex<Option<Child>>>,
    /// Session ID for tracking and debugging
    session_id: String,
    /// Process description for logging
    description: String,
    /// Whether termination should be performed
    enabled: bool,
}

impl ProcessGuard {
    /// Creates a new process guard for the given child process
    /// 
    /// # Arguments
    /// * `process` - Child process to manage
    /// * `session_id` - Unique identifier for tracking
    /// * `description` - Human-readable description of the process
    /// 
    /// # Example
    /// ```rust,no_run
    /// use std::process::{Command, Child};
    /// # use std::sync::{Arc, Mutex};
    /// # struct ProcessGuard {
    /// #     process: Arc<Mutex<Option<Child>>>,
    /// #     session_id: String,
    /// #     description: String,
    /// #     enabled: bool,
    /// # }
    /// # impl ProcessGuard {
    /// #     fn new(process: Child, session_id: String, description: String) -> Self {
    /// #         Self {
    /// #             process: Arc::new(Mutex::new(Some(process))),
    /// #             session_id,
    /// #             description,
    /// #             enabled: true,
    /// #         }
    /// #     }
    /// # }
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let child = Command::new("ffmpeg").spawn()?;
    /// let guard = ProcessGuard::new(child, "session-123".to_string(), "FFmpeg conversion".to_string());
    /// # Ok(())
    /// # }
    /// ```
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
    /// 
    /// This allows multiple threads to monitor or interact with the process
    /// while maintaining the cleanup guarantee.
    pub fn process_handle(&self) -> Arc<Mutex<Option<Child>>> {
        Arc::clone(&self.process)
    }
    
    /// Waits for the process to complete and returns the exit status
    /// 
    /// This consumes the process from the guard, preventing termination on drop.
    /// Use this when you want to wait for normal completion.
    /// 
    /// # Returns
    /// The process exit status, or an error if waiting failed
    pub fn wait(self) -> Result<std::process::ExitStatus> {
        debug!("Session {}: Waiting for process completion: {}", 
               self.session_id, self.description);
        
        let mut process_lock = self.process.lock()
            .map_err(|_| AppError::General("Failed to acquire process lock".to_string()))?;
        
        match process_lock.take() {
            Some(mut child) => {
                let status = child.wait()
                    .map_err(AppError::Io)?;
                
                debug!("Session {}: Process completed with status: {:?}", 
                       self.session_id, status);
                Ok(status)
            }
            None => {
                Err(AppError::General("Process already consumed".to_string()))
            }
        }
    }
    
    /// Attempts to terminate the process gracefully, then forcefully if needed
    /// 
    /// This method can be called manually to terminate the process before
    /// the guard is dropped.
    /// 
    /// # Returns
    /// `Ok(())` if termination succeeded, error otherwise
    pub fn terminate(&self) -> Result<()> {
        if !self.enabled {
            debug!("Session {}: Process termination disabled", self.session_id);
            return Ok(());
        }
        
        let mut process_lock = self.process.lock()
            .map_err(|_| AppError::General("Failed to acquire process lock".to_string()))?;
        
        match process_lock.as_mut() {
            Some(child) => {
                debug!("Session {}: Terminating process: {}", 
                       self.session_id, self.description);
                
                // Try graceful termination first
                if let Err(e) = child.kill() {
                    warn!("Session {}: Failed to kill process {}: {}", 
                          self.session_id, self.description, e);
                    return Err(AppError::General(format!("Process termination failed: {e}")));
                }
                
                // Wait for termination with timeout-like behavior
                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!("Session {}: Process terminated with status: {:?}", 
                               self.session_id, status);
                    }
                    Ok(None) => {
                        debug!("Session {}: Process termination initiated", self.session_id);
                    }
                    Err(e) => {
                        warn!("Session {}: Error checking process status: {}", 
                              self.session_id, e);
                    }
                }
                
                // Remove process from guard to prevent double-termination
                *process_lock = None;
                Ok(())
            }
            None => {
                debug!("Session {}: Process already terminated or consumed", self.session_id);
                Ok(())
            }
        }
    }
    
    /// Disables automatic termination for debugging
    pub fn disable_termination(&mut self) {
        debug!("Session {}: Process termination disabled for debugging", self.session_id);
        self.enabled = false;
    }
    
    /// Enables automatic termination (default state)
    pub fn enable_termination(&mut self) {
        debug!("Session {}: Process termination enabled", self.session_id);
        self.enabled = true;
    }
    
    /// Returns the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    /// Returns the process description
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl Drop for ProcessGuard {
    /// Automatically terminate the process when guard is dropped
    /// 
    /// This method never panics, even if termination fails. Errors are logged
    /// but not propagated to avoid issues during stack unwinding.
    fn drop(&mut self) {
        if !self.enabled {
            debug!("Session {}: Process termination disabled, skipping drop cleanup", 
                   self.session_id);
            return;
        }
        
        debug!("Session {}: Terminating process on drop: {}", 
               self.session_id, self.description);
        
        // Never panic in Drop - just log errors
        if let Err(e) = self.terminate() {
            error!("Session {}: Process termination failed during drop: {}", 
                   self.session_id, e);
        }
    }
}

/// Integration utilities for use with ProcessingContext
impl CleanupGuard {
    /// Creates a cleanup guard from a processing context
    /// 
    /// This is a convenience method for integration with the existing
    /// ProcessingContext pattern in the codebase.
    /// 
    /// # Arguments
    /// * `context` - Processing context containing session information
    pub fn from_context(context: &crate::audio::ProcessingContext) -> Self {
        Self::new(context.session.id())
    }
}

impl ProcessGuard {
    /// Creates a process guard from a processing context
    /// 
    /// # Arguments
    /// * `process` - Child process to manage
    /// * `context` - Processing context containing session information
    /// * `description` - Description of the process
    pub fn from_context(
        process: Child, 
        context: &crate::audio::ProcessingContext, 
        description: String
    ) -> Self {
        Self::new(process, context.session.id(), description)
    }
}
