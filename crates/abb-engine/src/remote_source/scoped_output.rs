use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

#[derive(Debug)]
pub(crate) struct StagedTempFile {
    final_path: PathBuf,
    partial_path: PathBuf,
    prepared: bool,
}

#[derive(Debug)]
pub(crate) struct ProvisionalCommittedFile {
    path: PathBuf,
    permanent: bool,
}

impl StagedTempFile {
    pub(crate) fn new(final_path: impl Into<PathBuf>) -> Self {
        let final_path = final_path.into();
        let partial_path = partial_sibling(&final_path);
        Self::from_paths(final_path, partial_path)
    }

    pub(crate) fn with_partial(
        final_path: impl Into<PathBuf>,
        partial_path: impl Into<PathBuf>,
    ) -> Self {
        Self::from_paths(final_path.into(), partial_path.into())
    }

    fn from_paths(final_path: PathBuf, partial_path: PathBuf) -> Self {
        debug_assert_ne!(
            final_path, partial_path,
            "StagedTempFile final and partial paths must be distinct"
        );
        Self {
            final_path,
            partial_path,
            prepared: false,
        }
    }

    pub(crate) fn partial_path(&self) -> &Path {
        &self.partial_path
    }

    /// Remove a stale partial sibling before writing. Must run before any partial write.
    pub(crate) fn prepare(&mut self) -> Result<()> {
        if self.prepared {
            return Ok(());
        }
        remove_if_present(&self.partial_path)?;
        self.prepared = true;
        Ok(())
    }

    pub(crate) async fn rename_and_commit(self, is_cancelled: impl Fn() -> bool) -> Result<()> {
        if is_cancelled() {
            return Err(AppError::cancelled());
        }
        ensure_same_parent(&self.partial_path, &self.final_path)?;
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
        if self.partial_path != self.final_path {
            let _ = remove_if_present(&self.partial_path);
        }
    }
}

impl Drop for ProvisionalCommittedFile {
    fn drop(&mut self) {
        if !self.permanent {
            let _ = remove_if_present(&self.path);
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

pub(crate) fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn ensure_same_parent(partial: &Path, final_path: &Path) -> Result<()> {
    let partial_parent = partial.parent().ok_or_else(|| {
        AppError::General("Staged partial path has no parent directory.".to_string())
    })?;
    let final_parent = final_path.parent().ok_or_else(|| {
        AppError::General("Staged final path has no parent directory.".to_string())
    })?;
    if partial_parent != final_parent {
        return Err(AppError::General(
            "Staged partial and final paths must share the same parent directory.".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "scoped_output_tests.rs"]
mod tests;
