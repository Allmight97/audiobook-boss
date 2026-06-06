//! Local staging for processor output artifacts.

use std::path::{Path, PathBuf};

use crate::errors::{sanitize_path_for_display, AppError, Result};
use uuid::Uuid;

const STAGING_DIR_PREFIX: &str = ".abb-processing-";
const PROCESSING_DIR: &str = "processing";
const SESSIONS_DIR: &str = "sessions";

pub(crate) fn workspace_root_for_app_cache(cache_dir: &Path) -> PathBuf {
    cache_dir.join(PROCESSING_DIR).join(SESSIONS_DIR)
}

pub(crate) fn create_processing_workspace_dir(
    session_id: Uuid,
    workspace_root: &Path,
) -> Result<PathBuf> {
    if workspace_root.as_os_str().is_empty() {
        return Err(AppError::TempDirectoryCreation(
            "Processing workspace root is empty.".to_string(),
        ));
    }

    std::fs::create_dir_all(workspace_root).map_err(|error| {
        AppError::TempDirectoryCreation(format!(
            "Cannot create processing workspace '{}': {}",
            sanitize_path_for_display(workspace_root),
            error
        ))
    })?;

    let staging_dir = workspace_root.join(format!("{STAGING_DIR_PREFIX}{session_id}"));
    std::fs::create_dir(&staging_dir).map_err(|error| {
        AppError::TempDirectoryCreation(format!(
            "Cannot create processing session workspace '{}': {}",
            sanitize_path_for_display(&staging_dir),
            error
        ))
    })?;

    log::info!(
        "processing_workspace kind=app-cache status=created root={} session_dir={}",
        workspace_root.display(),
        staging_dir.display()
    );

    Ok(staging_dir)
}

pub(crate) fn cleanup_abandoned_processing_sessions(workspace_root: &Path) -> Result<()> {
    if !workspace_root.exists() {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(workspace_root)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::ResourceCleanup(
            "Refusing to follow processing workspace symlink during cleanup".to_string(),
        ));
    }

    for entry in std::fs::read_dir(workspace_root)? {
        let path = entry?.path();
        purge_processing_session(workspace_root, &path)?;
    }

    Ok(())
}

fn ensure_owned_child(root: &Path, child: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .map_err(|error| AppError::ResourceCleanup(format!("Invalid processing root: {error}")))?;
    let child = child.canonicalize().map_err(|error| {
        AppError::ResourceCleanup(format!("Invalid processing workspace child: {error}"))
    })?;
    if child.starts_with(root) {
        return Ok(());
    }
    Err(AppError::ResourceCleanup(
        "Refusing to cleanup path outside ABB processing workspace root".to_string(),
    ))
}

pub(crate) fn purge_processing_session(workspace_root: &Path, session_dir: &Path) -> Result<()> {
    if !session_dir.exists() {
        return Ok(());
    }
    ensure_owned_child(workspace_root, session_dir)?;
    remove_owned_dir(session_dir)
}

fn remove_owned_dir(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::ResourceCleanup(
            "Refusing to follow processing workspace symlink during cleanup".to_string(),
        ));
    }
    std::fs::remove_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
// EXCEPTION: tiny private staging-path invariant tests; keeping them inline avoids exposing helper internals for external tests.
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_processing_workspace_under_supplied_local_root_not_final_parent() {
        let local = TempDir::new().expect("local temp root");
        let destination = TempDir::new().expect("destination temp root");
        let workspace_root = local.path().join("processing").join("sessions");
        let final_path = destination.path().join("nested").join("book.m4b");

        let session_id = Uuid::nil();
        let staging_dir =
            create_processing_workspace_dir(session_id, &workspace_root).expect("staging dir");

        assert!(staging_dir.starts_with(&workspace_root));
        assert!(!staging_dir.starts_with(final_path.parent().expect("final parent")));
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
        let workspace_root = root.path().join("processing").join("sessions");
        std::fs::create_dir_all(&workspace_root).expect("workspace root");
        let staging_dir = workspace_root.join(format!("{STAGING_DIR_PREFIX}{session_id}"));
        std::fs::write(&staging_dir, b"occupied").expect("write occupied staging file");

        let err = create_processing_workspace_dir(session_id, &workspace_root)
            .expect_err("existing staging entry should fail");

        assert!(
            err.to_string().contains("processing session workspace"),
            "error should identify staging creation"
        );
    }

    #[test]
    fn rejects_empty_workspace_root() {
        let err = create_processing_workspace_dir(Uuid::nil(), Path::new(""))
            .expect_err("empty workspace root should be rejected");

        assert!(
            err.to_string().contains("workspace root is empty"),
            "error should identify empty workspace root"
        );
    }

    #[test]
    fn cleanup_refuses_out_of_root_session() {
        let root = TempDir::new().expect("root");
        let outside = TempDir::new().expect("outside");
        let session = outside.path().join("session");
        std::fs::create_dir_all(&session).expect("session");

        let err = purge_processing_session(root.path(), &session)
            .expect_err("out of root cleanup should fail");

        assert!(
            err.to_string()
                .contains("outside ABB processing workspace root"),
            "unexpected error: {err}"
        );
        assert!(session.exists(), "outside session must not be removed");
    }

    #[test]
    fn cleanup_abandoned_processing_sessions_removes_owned_children_only() {
        let root = TempDir::new().expect("root");
        let session = root.path().join(".abb-processing-session");
        std::fs::create_dir_all(&session).expect("session");
        std::fs::write(session.join("worker-output.m4b"), b"audio").expect("worker output");

        cleanup_abandoned_processing_sessions(root.path()).expect("cleanup");

        assert!(root.path().exists(), "workspace root should remain");
        assert!(!session.exists(), "owned session should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_refuses_symlink_session() {
        let root = TempDir::new().expect("root");
        let target = root.path().join("target");
        let link = root.path().join("link");
        std::fs::create_dir_all(&target).expect("target");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let err =
            purge_processing_session(root.path(), &link).expect_err("symlink cleanup should fail");

        assert!(
            err.to_string().contains("symlink"),
            "unexpected error: {err}"
        );
        assert!(target.exists(), "target must not be removed");
    }
}
