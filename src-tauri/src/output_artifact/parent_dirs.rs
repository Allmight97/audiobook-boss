use super::plan::action_requires_output_write;
use super::types::ResolvedOutputPlan;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct OutputParentDirCleanup {
    output_root: PathBuf,
    created_dirs: Vec<PathBuf>,
    active: bool,
}

impl OutputParentDirCleanup {
    fn new(output_root: PathBuf) -> Self {
        Self {
            output_root,
            created_dirs: Vec::new(),
            active: true,
        }
    }

    fn add_created_dirs(&mut self, created_dirs: Vec<PathBuf>) {
        self.created_dirs.extend(created_dirs);
    }

    pub(crate) fn release(mut self) {
        self.active = false;
        self.created_dirs.clear();
    }

    pub(crate) fn cleanup_now(mut self) -> Result<()> {
        self.cleanup_active()
    }

    fn cleanup_active(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let mut first_error = None;
        for dir in std::mem::take(&mut self.created_dirs).into_iter().rev() {
            if let Err(error) = remove_created_empty_dir(&self.output_root, &dir) {
                log::warn!(
                    "output_parent_cleanup status=err dir={} err={}",
                    dir.display(),
                    error
                );
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for OutputParentDirCleanup {
    fn drop(&mut self) {
        if self.active && !self.created_dirs.is_empty() {
            if let Err(error) = self.cleanup_active() {
                log::warn!("output_parent_cleanup status=drop_err err={error}");
            }
        }
    }
}

pub(crate) fn ensure_output_parent_dirs<'a>(
    output_root: &Path,
    outputs: impl IntoIterator<Item = &'a ResolvedOutputPlan>,
) -> Result<OutputParentDirCleanup> {
    let output_root = output_root.canonicalize().map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot validate output root '{}': {}",
            sanitize_path_for_display(output_root),
            error
        ))
    })?;
    let mut cleanup = OutputParentDirCleanup::new(output_root);

    for output in outputs {
        if !action_requires_output_write(output.action) {
            continue;
        }
        if let Some(parent) = output.resolved_path.parent() {
            ensure_output_parent_under_root(&cleanup.output_root, parent)?;
            let created_dirs = create_missing_output_parent_dirs(parent)?;
            cleanup.add_created_dirs(created_dirs);
        }
    }

    Ok(cleanup)
}

fn create_missing_output_parent_dirs(parent: &Path) -> Result<Vec<PathBuf>> {
    let mut missing_dirs = Vec::new();
    let mut current = Some(parent);
    while let Some(path) = current {
        if path.try_exists().map_err(AppError::Io)? {
            break;
        }
        missing_dirs.push(path.to_path_buf());
        current = path.parent();
    }

    let mut created_dirs = Vec::new();
    for dir in missing_dirs.into_iter().rev() {
        match std::fs::create_dir(&dir) {
            Ok(()) => created_dirs.push(dir),
            Err(error) if error.kind() == ErrorKind::AlreadyExists && dir.is_dir() => {}
            Err(error) => {
                return Err(AppError::FileValidation(format!(
                    "Cannot create output directory '{}': {}",
                    sanitize_path_for_display(&dir),
                    error
                )));
            }
        }
    }

    Ok(created_dirs)
}

fn ensure_output_parent_under_root(output_root: &Path, parent: &Path) -> Result<()> {
    let Some(existing_ancestor) = nearest_existing_ancestor(parent)? else {
        return Err(AppError::FileValidation(format!(
            "Cannot validate output directory '{}'.",
            sanitize_path_for_display(parent)
        )));
    };
    let existing_ancestor = existing_ancestor.canonicalize().map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot validate output directory '{}': {}",
            sanitize_path_for_display(&existing_ancestor),
            error
        ))
    })?;

    if existing_ancestor.starts_with(output_root) {
        return Ok(());
    }

    Err(AppError::FileValidation(format!(
        "Output directory '{}' escapes the configured output root.",
        sanitize_path_for_display(parent)
    )))
}

fn nearest_existing_ancestor(path: &Path) -> Result<Option<PathBuf>> {
    let mut current = Some(path);
    while let Some(path) = current {
        if path.try_exists().map_err(AppError::Io)? {
            return Ok(Some(path.to_path_buf()));
        }
        current = path.parent();
    }
    Ok(None)
}

fn remove_created_empty_dir(output_root: &Path, dir: &Path) -> Result<()> {
    if !dir.try_exists().map_err(AppError::Io)? {
        return Ok(());
    }

    let metadata = std::fs::symlink_metadata(dir).map_err(AppError::Io)?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::ResourceCleanup(
            "Refusing to cleanup symlinked output directory".to_string(),
        ));
    }

    let canonical_dir = dir.canonicalize().map_err(|error| {
        AppError::ResourceCleanup(format!("Invalid output cleanup directory: {error}"))
    })?;
    if canonical_dir == output_root || !canonical_dir.starts_with(output_root) {
        return Err(AppError::ResourceCleanup(
            "Refusing to cleanup output directory outside configured output root".to_string(),
        ));
    }

    match std::fs::remove_dir(dir) {
        Ok(()) => {
            log::info!("output_parent_cleanup status=removed dir={}", dir.display());
            Ok(())
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(AppError::ResourceCleanup(format!(
            "Failed to remove output directory '{}': {}",
            sanitize_path_for_display(dir),
            error
        ))),
    }
}

#[cfg(test)]
#[path = "parent_dirs_tests.rs"]
mod tests;
