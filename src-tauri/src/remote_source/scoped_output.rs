use std::path::{Path, PathBuf};

/// RAII guard for a staged output file and its intermediate (`.partial`/temp)
/// sibling. On drop it best-effort removes the partial path always, and the
/// final path unless [`StagedTempFile::commit`] was called.
///
/// This replaces the per-branch `remove_file` cleanup that was previously
/// hand-copied across every error path of the supplemental-PDF downloader and
/// the materializer: cleanup now happens on every early return, `?`, or panic
/// without per-branch bookkeeping. Removal is synchronous (best-effort), which
/// matches the existing cleanup helpers that already use `std::fs::remove_file`
/// inside async code.
pub(crate) struct StagedTempFile {
    final_path: PathBuf,
    partial_path: PathBuf,
    committed: bool,
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
        }
    }

    pub(crate) fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub(crate) fn partial_path(&self) -> &Path {
        &self.partial_path
    }

    /// Keep the final path: drop will no longer remove it. The partial sibling
    /// is still cleaned on drop.
    pub(crate) fn commit(mut self) {
        self.committed = true;
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

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn partial_sibling(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy())
        .map(|extension| format!("{extension}.partial"))
        .unwrap_or_else(|| "partial".to_string());
    path.with_extension(extension)
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
            std::fs::write(staged.final_path(), b"final").expect("write final");
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
