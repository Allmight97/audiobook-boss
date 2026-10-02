use super::types::ResolvedOutputPlan;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Owns every directory execution created for its outputs, including a missing
/// output root, until the run releases them or rolls them back.
#[derive(Debug)]
pub(crate) struct OutputParentDirCleanup {
    /// Canonical directory that existed before execution created anything; only
    /// directories strictly below it can be removed.
    existing_anchor: PathBuf,
    created_dirs: Vec<PathBuf>,
    /// Each output's parent folder, in the order the outputs were given;
    /// `None` for an output that writes nothing.
    title_parents: Vec<Option<PathBuf>>,
    active: bool,
}

impl OutputParentDirCleanup {
    fn new(existing_anchor: PathBuf) -> Self {
        Self {
            existing_anchor,
            created_dirs: Vec::new(),
            title_parents: Vec::new(),
            active: true,
        }
    }

    /// Keeps every folder: the run published what it planned.
    pub(crate) fn release(&mut self) {
        self.active = false;
        self.created_dirs.clear();
    }

    /// Removes every created folder that is empty.
    pub(crate) fn cleanup_now(&mut self) -> Result<()> {
        self.cleanup_active()
    }

    /// Removes the empty folders created for the output at `position` alone,
    /// after it ended without publishing. A folder another output sits in or
    /// under stays for that output; the run's own cleanup sees it later.
    pub(crate) fn cleanup_title(&self, position: usize) -> Result<()> {
        let Some(Some(parent)) = self.title_parents.get(position).filter(|_| self.active) else {
            return Ok(());
        };
        let shared = |dir: &Path| {
            self.title_parents.iter().enumerate().any(|(index, other)| {
                index != position && other.as_ref().is_some_and(|other| other.starts_with(dir))
            })
        };
        let mut first_error = None;
        for dir in self
            .created_dirs
            .iter()
            .rev()
            .filter(|dir| parent.starts_with(dir) && !shared(dir))
        {
            if let Err(error) = remove_created_empty_dir(&self.existing_anchor, dir) {
                log::warn!(
                    "output_parent_cleanup status=title_err dir={} err={}",
                    dir.display(),
                    error
                );
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn cleanup_active(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let mut first_error = None;
        for dir in std::mem::take(&mut self.created_dirs).into_iter().rev() {
            if let Err(error) = remove_created_empty_dir(&self.existing_anchor, &dir) {
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

/// Creates the output root and each writable output's parent after review has
/// passed. Every directory is registered as soon as it exists, so an error part
/// way through drops the returned guard and removes what this call created.
pub(crate) fn ensure_output_parent_dirs<'a>(
    output_root: &Path,
    outputs: impl IntoIterator<Item = &'a ResolvedOutputPlan>,
) -> Result<OutputParentDirCleanup> {
    let existing_anchor = nearest_existing_ancestor(output_root)?
        .and_then(|path| path.canonicalize().ok())
        .ok_or_else(|| {
            AppError::FileValidation(format!(
                "Cannot validate output root '{}'.",
                sanitize_path_for_display(output_root)
            ))
        })?;
    let mut cleanup = OutputParentDirCleanup::new(existing_anchor);
    create_missing_dirs(output_root, &mut cleanup)?;
    let output_root = output_root.canonicalize().map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot validate output root '{}': {}",
            sanitize_path_for_display(output_root),
            error
        ))
    })?;

    for output in outputs {
        let parent = output
            .resolved_path
            .parent()
            .filter(|_| abb_output_artifact_core::action_requires_output_write(output.action));
        cleanup.title_parents.push(parent.map(Path::to_path_buf));
        if let Some(parent) = parent {
            ensure_output_parent_under_root(&output_root, parent)?;
            create_missing_dirs(parent, &mut cleanup)?;
        }
    }

    Ok(cleanup)
}

fn create_missing_dirs(target: &Path, cleanup: &mut OutputParentDirCleanup) -> Result<()> {
    let mut missing_dirs = Vec::new();
    let mut current = Some(target);
    while let Some(path) = current {
        if path.try_exists().map_err(AppError::Io)? {
            break;
        }
        missing_dirs.push(path.to_path_buf());
        current = path.parent();
    }

    for dir in missing_dirs.into_iter().rev() {
        match std::fs::create_dir(&dir) {
            Ok(()) => cleanup.created_dirs.push(dir),
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

    Ok(())
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

fn remove_created_empty_dir(existing_anchor: &Path, dir: &Path) -> Result<()> {
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
    if canonical_dir == existing_anchor || !canonical_dir.starts_with(existing_anchor) {
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
