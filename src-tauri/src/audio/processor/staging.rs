//! Destination-adjacent staging for processor output artifacts.

use std::path::{Path, PathBuf};

use crate::errors::{sanitize_path_for_display, AppError, Result};
use uuid::Uuid;

const STAGING_DIR_PREFIX: &str = ".abb-processing-";

pub(crate) fn create_destination_staging_dir(
    session_id: Uuid,
    final_artifact_path: &Path,
) -> Result<PathBuf> {
    let parent = final_artifact_path.parent().ok_or_else(|| {
        AppError::FileValidation(format!(
            "Output path '{}' has no parent directory.",
            sanitize_path_for_display(final_artifact_path)
        ))
    })?;

    std::fs::create_dir_all(parent).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot create output directory '{}': {}",
            sanitize_path_for_display(parent),
            error
        ))
    })?;

    let staging_dir = parent.join(format!("{STAGING_DIR_PREFIX}{session_id}"));
    std::fs::create_dir(&staging_dir).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot create output staging directory '{}': {}",
            sanitize_path_for_display(&staging_dir),
            error
        ))
    })?;

    Ok(staging_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_staging_directory_beside_final_artifact() {
        let root = TempDir::new().expect("temp root");
        let final_path = root.path().join("nested").join("book.m4b");

        let session_id = Uuid::nil();
        let staging_dir =
            create_destination_staging_dir(session_id, &final_path).expect("staging dir");

        assert_eq!(
            staging_dir.parent().expect("staging parent"),
            final_path.parent().expect("final parent")
        );
        assert!(staging_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("staging filename")
            .starts_with(STAGING_DIR_PREFIX));
        assert!(staging_dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("staging filename")
            .ends_with(&session_id.to_string()));
        assert!(staging_dir.is_dir());
    }

    #[test]
    fn rejects_existing_staging_entry() {
        let root = TempDir::new().expect("temp root");
        let session_id = Uuid::nil();
        let final_path = root.path().join("book.m4b");
        let staging_dir = root
            .path()
            .join(format!("{STAGING_DIR_PREFIX}{session_id}"));
        std::fs::write(&staging_dir, b"occupied").expect("write occupied staging file");

        let err = create_destination_staging_dir(session_id, &final_path)
            .expect_err("existing staging entry should fail");

        assert!(
            err.to_string().contains("staging directory"),
            "error should identify staging creation"
        );
    }
}
