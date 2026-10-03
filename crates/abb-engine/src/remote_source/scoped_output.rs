use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

/// RAII guard for a staged output file and its intermediate (`.partial`/temp)
/// sibling. Callers must follow the owned lifecycle:
/// `prepare` → write partial → `rename_and_commit` (or drop cleans uncommitted).
///
/// On drop, the partial path is always removed when distinct from the final path.
/// The final path is removed unless the guard was committed via
/// [`StagedTempFile::rename_and_commit`] or the test-only [`StagedTempFile::commit`].
#[derive(Debug)]
pub(crate) struct StagedTempFile {
    final_path: PathBuf,
    partial_path: PathBuf,
    committed: bool,
    prepared: bool,
}

/// Holds a committed staged file until validation and downstream acquisition
/// steps succeed. Drop removes the file unless [`ProvisionalCommittedFile::permanent`]
/// was called.
#[derive(Debug)]
pub(crate) struct ProvisionalCommittedFile {
    path: PathBuf,
    permanent: bool,
}

impl StagedTempFile {
    /// Guard a staged output whose intermediate sibling is `<final>.partial`
    /// (or `partial` when the path has no extension).
    pub(crate) fn new(final_path: impl Into<PathBuf>) -> Self {
        let final_path = final_path.into();
        let partial_path = partial_sibling(&final_path);
        Self::from_paths(final_path, partial_path)
    }

    /// Guard a staged output with an explicit intermediate/temp path (e.g. the
    /// materializer's `output_temp_path`).
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
            committed: false,
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

    /// Cancel check → same-directory rename partial→final → commit. Consumes the guard
    /// lifecycle: after success the final path survives drop.
    pub(crate) async fn rename_and_commit(mut self, is_cancelled: impl Fn() -> bool) -> Result<()> {
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
        tokio::fs::rename(&self.partial_path, &self.final_path)
            .await
            .map_err(|error| {
                AppError::General(format!(
                    "Staged output rename failed (same-directory required): {error}"
                ))
            })?;
        self.committed = true;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn commit(mut self) {
        self.committed = true;
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
        if !self.committed {
            let _ = remove_if_present(&self.final_path);
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

/// Remove a committed staged file when a later acquisition step fails after commit.
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn drop_removes_partial_and_final_when_not_committed() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.pdf");
        std::fs::write(&final_path, b"final").expect("write final");
        let partial_path;
        {
            let staged = StagedTempFile::new(&final_path);
            partial_path = staged.partial_path().to_path_buf();
            std::fs::write(staged.partial_path(), b"partial").expect("write partial");
            assert_eq!(partial_path, root.path().join("book.pdf.partial"));
        }
        assert!(!final_path.exists());
        assert!(!partial_path.exists());
    }

    #[test]
    fn commit_keeps_final_but_still_removes_partial() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.pdf");
        let partial_path;
        {
            let staged = StagedTempFile::new(&final_path);
            partial_path = staged.partial_path().to_path_buf();
            std::fs::write(staged.partial_path(), b"partial").expect("write partial");
            std::fs::write(&final_path, b"final").expect("write final");
            staged.commit();
        }
        assert!(final_path.exists());
        assert!(!partial_path.exists());
    }

    #[test]
    fn with_partial_uses_explicit_temp_path() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("out.m4b");
        let temp_path = root.path().join("out.m4b.partial");
        std::fs::write(&temp_path, b"temp").expect("write temp");
        std::fs::write(&final_path, b"final").expect("write final");
        {
            let _staged = StagedTempFile::with_partial(&final_path, &temp_path);
        }
        assert!(!final_path.exists());
        assert!(!temp_path.exists());
    }

    #[test]
    fn no_extension_partial_is_named_partial() {
        let staged = StagedTempFile::new(Path::new("/tmp/source"));
        assert_eq!(staged.partial_path(), Path::new("/tmp/source.partial"));
    }

    #[test]
    fn prepare_removes_stale_partial_before_write() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.m4b");
        let partial_path = partial_sibling(&final_path);
        std::fs::write(&partial_path, b"stale").expect("write stale partial");
        let mut staged = StagedTempFile::new(&final_path);
        staged.prepare().expect("prepare");
        assert!(!partial_path.exists());
    }

    #[tokio::test]
    async fn rename_and_commit_keeps_final_and_drops_partial() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.m4b");
        let partial_path = partial_sibling(&final_path);
        let mut staged = StagedTempFile::new(&final_path);
        staged.prepare().expect("prepare");
        std::fs::write(staged.partial_path(), b"payload").expect("write partial");
        staged
            .rename_and_commit(|| false)
            .await
            .expect("rename and commit");
        assert!(final_path.exists());
        assert!(!partial_path.exists());
    }

    #[tokio::test]
    async fn rename_and_commit_cancelled_removes_both_paths() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.m4b");
        let partial_path = partial_sibling(&final_path);
        let mut staged = StagedTempFile::new(&final_path);
        staged.prepare().expect("prepare");
        std::fs::write(staged.partial_path(), b"payload").expect("write partial");
        let error = staged
            .rename_and_commit(|| true)
            .await
            .expect_err("cancelled");
        assert!(matches!(error, AppError::Cancellation(_)));
        assert!(!final_path.exists());
        assert!(!partial_path.exists());
    }

    #[tokio::test]
    async fn rename_and_commit_errors_when_partial_missing_before_commit() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("book.m4b");
        let mut staged = StagedTempFile::new(&final_path);
        staged.prepare().expect("prepare");
        // The staged write existed after prepare(), then vanished before commit.
        std::fs::write(staged.partial_path(), b"payload").expect("write partial");
        std::fs::remove_file(staged.partial_path()).expect("remove partial");

        let error = staged
            .rename_and_commit(|| false)
            .await
            .expect_err("a missing partial must fail the commit");

        assert!(
            matches!(&error, AppError::General(message)
                if message == "Staged partial output is missing before commit."),
            "unexpected error: {error:?}"
        );
        assert!(
            !final_path.exists(),
            "no final file may appear on a failed commit"
        );
    }

    #[tokio::test]
    async fn rename_and_commit_rejects_cross_parent_partial_and_final() {
        let root = TempDir::new().expect("temp root");
        let final_dir = root.path().join("final");
        let partial_dir = root.path().join("staging");
        std::fs::create_dir_all(&final_dir).expect("final dir");
        std::fs::create_dir_all(&partial_dir).expect("partial dir");
        let final_path = final_dir.join("book.m4b");
        let partial_path = partial_dir.join("book.m4b.partial");

        // The same-parent guard runs before the existence check, so no partial
        // is written: a cross-device rename fallback must never be attempted.
        let staged = StagedTempFile::with_partial(&final_path, &partial_path);
        let error = staged
            .rename_and_commit(|| false)
            .await
            .expect_err("a cross-parent staged write must be rejected");

        assert!(
            matches!(&error, AppError::General(message)
                if message == "Staged partial and final paths must share the same parent directory."),
            "unexpected error: {error:?}"
        );
        assert!(
            !final_path.exists(),
            "no final file may appear on a rejected commit"
        );
    }

    #[test]
    fn provisional_committed_file_drops_unless_permanent() {
        let root = TempDir::new().expect("temp root");
        let path = root.path().join("book.m4b");
        std::fs::write(&path, b"final").expect("write");
        {
            let guard = ProvisionalCommittedFile::new(path.clone());
            assert!(guard.path().exists());
        }
        assert!(!path.exists());

        std::fs::write(&path, b"final").expect("rewrite");
        {
            let guard = ProvisionalCommittedFile::new(path.clone());
            guard.permanent();
        }
        assert!(path.exists());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "StagedTempFile final and partial paths must be distinct")]
    fn with_partial_rejects_matching_paths_in_debug() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("out.m4b");

        let _staged = StagedTempFile::with_partial(&final_path, &final_path);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_extension_still_gets_partial_suffix() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = TempDir::new().expect("temp root");
        let file_name = OsString::from_vec(b"book.\xFFpdf".to_vec());
        let final_path = root.path().join(file_name);
        let staged = StagedTempFile::new(&final_path);

        assert!(staged
            .partial_path()
            .to_string_lossy()
            .ends_with(".partial"));
    }
}
