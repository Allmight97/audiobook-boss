use super::types::ResolvedOutputPlan;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

/// The parent folder of every output, in every run, that has not ended yet.
/// No cleanup removes a folder one of them will still write into, whichever
/// run created it.
#[derive(Default)]
struct DirectoryOwnership {
    claims: Vec<PathBuf>,
    /// Kept across runs until the final claimant ends; failed removals retry.
    created: Vec<(PathBuf, PathBuf)>,
}

static OWNERSHIP: LazyLock<Mutex<DirectoryOwnership>> = LazyLock::new(Mutex::default);

fn ownership() -> std::sync::MutexGuard<'static, DirectoryOwnership> {
    OWNERSHIP.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Called under the ownership lock, including filesystem removal, so a new
/// run cannot claim a directory between its last-claim check and removal.
fn prune_unclaimed(owned: &mut DirectoryOwnership, parents: &[PathBuf]) -> Result<()> {
    owned
        .created
        .sort_by_key(|(dir, _)| std::cmp::Reverse(dir.components().count()));
    let mut first_error = None;
    owned.created.retain(|(dir, anchor)| {
        if !parents.iter().any(|parent| parent.starts_with(dir))
            || claimed_under(&owned.claims, dir)
        {
            return true;
        }
        if let Err(error) = remove_created_empty_dir(anchor, dir) {
            log::warn!(
                "output_parent_cleanup status=err dir={} err={error}",
                dir.display()
            );
            first_error.get_or_insert(error);
            return true;
        }
        false
    });
    first_error.map_or(Ok(()), Err)
}

fn claimed_under(claims: &[PathBuf], dir: &Path) -> bool {
    claims.iter().any(|parent| parent.starts_with(dir))
}

/// Owns every directory execution created for its outputs, including a missing
/// output root, until the run releases them or rolls them back.
#[derive(Debug)]
pub(crate) struct OutputParentDirCleanup {
    /// Canonical directory that existed before execution created anything; only
    /// directories strictly below it can be removed.
    existing_anchor: PathBuf,
    created_dirs: Vec<PathBuf>,
    /// Each output's parent folder, in the order the outputs were given;
    /// `None` for an output that writes nothing or has ended. A `Some` is a
    /// claim in `OWNERSHIP`.
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
        let mut owned = ownership();
        owned
            .created
            .retain(|(dir, _)| !self.created_dirs.contains(dir));
        drop(owned);
        self.release_claims();
        self.active = false;
        self.created_dirs.clear();
    }

    /// Removes every created folder that is empty.
    pub(crate) fn cleanup_now(&mut self) -> Result<()> {
        self.cleanup_active()
    }

    /// Ends the output at `position`. When it did not publish, removes the
    /// empty folders created for it that no output still running writes
    /// into; the run's own cleanup sees the rest later.
    pub(crate) fn end_title(&mut self, position: usize, published: bool) -> Result<()> {
        let Some(parent) = self.title_parents.get_mut(position).and_then(Option::take) else {
            return Ok(());
        };
        release_claim(&parent);
        if published {
            ownership()
                .created
                .retain(|(dir, _)| !parent.starts_with(dir));
            return Ok(());
        }
        if !self.active {
            return Ok(());
        }
        prune_unclaimed(&mut ownership(), &[parent])
    }

    fn release_claims(&mut self) {
        for parent in self.title_parents.iter_mut().filter_map(Option::take) {
            release_claim(&parent);
        }
    }

    fn cleanup_active(&mut self) -> Result<()> {
        let mut parents = self.created_dirs.clone();
        parents.extend(self.title_parents.iter().flatten().cloned());
        self.release_claims();
        if !self.active {
            return Ok(());
        }
        self.active = false;

        self.created_dirs.clear();
        prune_unclaimed(&mut ownership(), &parents)
    }
}

fn release_claim(parent: &Path) {
    let mut owned = ownership();
    if let Some(index) = owned.claims.iter().position(|claimed| claimed == parent) {
        owned.claims.swap_remove(index);
    }
}

impl Drop for OutputParentDirCleanup {
    fn drop(&mut self) {
        if self.active {
            if let Err(error) = self.cleanup_active() {
                log::warn!("output_parent_cleanup status=drop_err err={error}");
            }
        } else {
            self.release_claims();
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
    let mut owned = ownership();
    create_missing_dirs(output_root, &mut cleanup, &mut owned)?;
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
            owned.claims.push(parent.to_path_buf());
            ensure_output_parent_under_root(&output_root, parent)?;
            create_missing_dirs(parent, &mut cleanup, &mut owned)?;
        }
    }

    drop(owned);
    Ok(cleanup)
}

fn create_missing_dirs(
    target: &Path,
    cleanup: &mut OutputParentDirCleanup,
    owned: &mut DirectoryOwnership,
) -> Result<()> {
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
            Ok(()) => {
                owned
                    .created
                    .push((dir.clone(), cleanup.existing_anchor.clone()));
                cleanup.created_dirs.push(dir);
            }
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
