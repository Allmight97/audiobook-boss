use std::path::{Path, PathBuf};

use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::owned_dir::OwnedRoot;
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
        sanitize_path_for_display(workspace_root),
        sanitize_path_for_display(&staging_dir)
    );

    Ok(staging_dir)
}

fn owned_root(workspace_root: &Path) -> OwnedRoot<'_> {
    OwnedRoot {
        path: workspace_root,
        label: "processing workspace",
    }
}

/// Startup policy: sweep each abandoned session but keep the workspace root.
pub(crate) fn cleanup_abandoned_processing_sessions(workspace_root: &Path) -> Result<()> {
    if !workspace_root.exists() {
        return Ok(());
    }
    owned_root(workspace_root).ensure_root_not_symlink()?;

    for entry in std::fs::read_dir(workspace_root)? {
        let path = entry?.path();
        log::info!(
            "media_cleanup reason=startup_sweep pid={} session_dir={:?} artifact={}",
            std::process::id(),
            sanitize_path_for_display(&path),
            crate::diagnostics::artifact_id(&path)
        );
        crate::diagnostics::stage("startup_cleanup", &path, || {
            purge_processing_session(workspace_root, &path)
        })?;
    }

    Ok(())
}

pub(crate) fn purge_processing_session(workspace_root: &Path, session_dir: &Path) -> Result<()> {
    owned_root(workspace_root).remove_child(session_dir)
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
