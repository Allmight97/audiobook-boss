use crate::errors::{sanitize_path_for_display, AppError, Result};
use log::debug;
use std::path::{Path, PathBuf};

use super::CleanupGuard;

impl CleanupGuard {
    /// Internal cleanup implementation that never panics
    pub(crate) fn perform_cleanup(&self, paths: &[PathBuf]) -> Result<()> {
        let mut first_error: Option<AppError> = None;

        for path in paths {
            if let Err(e) = self.cleanup_single_path(path) {
                log::error!(
                    "Session {}: Failed to cleanup {}: {}",
                    self.raw_session_id(),
                    sanitize_path_for_display(path),
                    e
                );

                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }

        match first_error {
            Some(err) => Err(err),
            None => {
                debug!(
                    "Session {}: All cleanup operations completed successfully",
                    self.raw_session_id()
                );
                Ok(())
            }
        }
    }

    /// Clean up a single path (file or directory)
    pub(crate) fn cleanup_single_path(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!(
                "Session {}: Path already removed: {}",
                self.raw_session_id(),
                sanitize_path_for_display(path)
            );
            return Ok(());
        }

        if path.is_dir() {
            debug!(
                "Session {}: Removing directory: {}",
                self.raw_session_id(),
                sanitize_path_for_display(path)
            );
            std::fs::remove_dir_all(path).map_err(AppError::Io)?;
        } else {
            debug!(
                "Session {}: Removing file: {}",
                self.raw_session_id(),
                sanitize_path_for_display(path)
            );
            std::fs::remove_file(path).map_err(AppError::Io)?;
        }

        Ok(())
    }
}
