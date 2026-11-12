use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

#[derive(Debug)]
pub(crate) struct StagedTempFile {
    final_path: PathBuf,
    partial_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ProvisionalCommittedFile {
    path: PathBuf,
    permanent: bool,
}

impl StagedTempFile {
    pub(crate) fn create(final_path: impl Into<PathBuf>) -> Result<Self> {
        let final_path = final_path.into();
        let partial_path = partial_sibling(&final_path);
        remove_if_present(&partial_path)?;
        Ok(Self {
            partial_path,
            final_path,
        })
    }

    pub(crate) fn partial_path(&self) -> &Path {
        &self.partial_path
    }

    pub(crate) async fn rename_and_commit(self, is_cancelled: impl Fn() -> bool) -> Result<()> {
        if is_cancelled() {
            return Err(super::cancellation::remote_acquisition_cancelled());
        }
        if !tokio::fs::try_exists(&self.partial_path)
            .await
            .map_err(|error| {
                AppError::General(format!(
                    "Staged partial output check failed before commit: {error}"
                ))
            })?
        {
            return Err(AppError::General(
                "Staged partial output is missing before commit.".to_string(),
            ));
        }
        #[expect(
            clippy::disallowed_methods,
            reason = "publishes a download partial under its name in ABB's own staging folder, never over a user's file"
        )]
        tokio::fs::rename(&self.partial_path, &self.final_path)
            .await
            .map_err(|error| {
                AppError::General(format!(
                    "Staged output rename failed (same-directory required): {error}"
                ))
            })?;
        Ok(())
    }
}

impl ProvisionalCommittedFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            permanent: false,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn permanent(mut self) {
        self.permanent = true;
    }
}

impl Drop for StagedTempFile {
    fn drop(&mut self) {
        remove_logging_failure(&self.partial_path);
    }
}

impl Drop for ProvisionalCommittedFile {
    fn drop(&mut self) {
        if !self.permanent {
            remove_logging_failure(&self.path);
        }
    }
}

pub(crate) fn rollback_committed_file(path: &Path) -> Result<()> {
    remove_if_present(path).map_err(Into::into)
}

pub(crate) fn partial_sibling(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy())
        .map(|extension| format!("{extension}.partial"))
        .unwrap_or_else(|| "partial".to_string());
    path.with_extension(extension)
}

pub(crate) fn remove_logging_failure(path: &Path) {
    if let Err(error) = remove_if_present(path) {
        log::warn!("remote_source could not remove a staged file: {error}");
    }
}

/// Waits between purge attempts; transient locks (AV scans, indexing) and a
/// stopped worker's last write usually clear within this window.
pub(crate) const PURGE_RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(400),
];

/// Runs `purge` until it succeeds or the retry delays run out, and returns the
/// last attempt's result.
pub(crate) async fn purge_with_retries<E>(
    mut purge: impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    let mut result = purge();
    for delay in PURGE_RETRY_DELAYS {
        if result.is_ok() {
            break;
        }
        tokio::time::sleep(delay).await;
        result = purge();
    }
    result
}

pub(crate) fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "scoped_output_tests.rs"]
mod tests;
