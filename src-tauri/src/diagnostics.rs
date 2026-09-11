//! Shared diagnostic records; never changes an operation's result or file ownership.
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Instant;

use crate::errors::{sanitize_path_for_display, Result};

pub(crate) fn version_label(version: u32) -> String {
    format!(
        "{}.{}.{}",
        version >> 16,
        (version >> 8) & 255,
        version & 255
    )
}

pub(crate) fn artifact_id(path: &Path) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hash);
    format!("{:016x}", hash.finish())
}

pub(crate) fn file_state(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(metadata) => format!(
            "exists=true bytes={} is_file={}",
            metadata.len(),
            metadata.is_file()
        ),
        Err(error) => format!(
            "exists={} stat_error={:?} errno={:?}",
            if error.kind() == std::io::ErrorKind::NotFound {
                "false"
            } else {
                "unknown"
            },
            error.kind(),
            error.raw_os_error()
        ),
    }
}

pub(crate) struct Stage<'a> {
    name: &'a str,
    path: &'a Path,
    artifact: String,
    started: Instant,
}

impl<'a> Stage<'a> {
    pub(crate) fn start(name: &'a str, path: &'a Path) -> Self {
        let artifact = artifact_id(path);
        log::info!(
            "media_stage stage={name} status=start artifact={artifact} parent_artifact={} pid={} file={:?} {}",
            path.parent().map(artifact_id).unwrap_or_else(|| "none".into()),
            std::process::id(),
            sanitize_path_for_display(path),
            file_state(path)
        );
        Self {
            name,
            path,
            artifact,
            started: Instant::now(),
        }
    }

    pub(crate) fn finish<T>(self, result: Result<T>) -> Result<T> {
        let Self {
            name,
            path,
            artifact,
            started,
        } = self;
        match &result {
            Ok(_) => log::info!("media_stage stage={name} status=ok artifact={artifact} elapsed_ms={} {}",
                started.elapsed().as_millis(), file_state(path)),
            Err(crate::errors::AppError::Cancellation(_)) => log::info!("media_stage stage={name} status=cancelled artifact={artifact} elapsed_ms={} {}",
                started.elapsed().as_millis(), file_state(path)),
            Err(error) => log::error!("media_stage stage={name} status=error artifact={artifact} elapsed_ms={} {} parent_state={:?} error={error}",
                started.elapsed().as_millis(), file_state(path), path.parent().map(file_state)),
        }
        result
    }
}

pub(crate) fn stage<T>(
    name: &str,
    path: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let record = Stage::start(name, path);
    record.finish(operation())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_snapshot_distinguishes_missing_empty_and_written_artifacts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("output.m4b");
        assert!(file_state(&path).contains("exists=false"));
        std::fs::write(&path, []).expect("empty artifact");
        assert!(file_state(&path).contains("bytes=0"));
        std::fs::write(&path, [1, 2, 3]).expect("written artifact");
        assert!(file_state(&path).contains("bytes=3"));
        assert_ne!(
            artifact_id(&path),
            artifact_id(&dir.path().join("another.m4b"))
        );
    }
}
