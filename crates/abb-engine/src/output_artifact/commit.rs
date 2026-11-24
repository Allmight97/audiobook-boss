use super::collision::path_entry_exists;
use super::types::{OutputKind, PlannedOutputAction};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::file_replace::SiblingTemp;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn collision_state_changed_error(final_path: &Path) -> AppError {
    AppError::FileValidation(format!(
        "Output collision state changed for '{}'. Review collisions and try again.",
        sanitize_path_for_display(final_path)
    ))
}

fn destination_refused_commit_error(final_path: &Path, error: impl std::fmt::Display) -> AppError {
    AppError::FileValidation(format!(
        "The destination refused the final file commit for '{}': {error}. \
         If the output folder is a network share, external drive, or \
         cloud-synced folder, check write permissions and free space, or \
         retry with a local output folder.",
        sanitize_path_for_display(final_path),
    ))
}

fn copy_staged_output_to_new_file(temp_output: &Path, destination_path: &Path) -> Result<()> {
    let mut source = std::fs::File::open(temp_output).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot open temporary output '{}' for final copy: {}",
            sanitize_path_for_display(temp_output),
            error
        ))
    })?;
    std::fs::File::create_new(destination_path)
        .and_then(|mut destination| {
            std::io::copy(&mut source, &mut destination)?;
            destination.sync_all()
        })
        .map_err(|error| destination_refused_commit_error(destination_path, error))
}

fn flush_staged_output(temp_output: &Path) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(temp_output)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot flush temporary output '{}' before publishing: {}",
                sanitize_path_for_display(temp_output),
                error
            ))
        })
}

fn link_without_replacing(source: &Path, final_path: &Path) -> Result<bool> {
    match std::fs::hard_link(source, final_path) {
        Ok(()) => Ok(true),
        Err(error) => classify_link_failure(&error, final_path),
    }
}

fn classify_link_failure(error: &std::io::Error, final_path: &Path) -> Result<bool> {
    if matches!(
        error.kind(),
        ErrorKind::AlreadyExists | ErrorKind::PermissionDenied
    ) && path_entry_exists(final_path)?
    {
        return Err(collision_state_changed_error(final_path));
    }
    if !matches!(
        error.kind(),
        ErrorKind::CrossesDevices | ErrorKind::PermissionDenied | ErrorKind::Unsupported
    ) {
        log::warn!(
            "finalize_move method=hard-link status=err dest={} err={}",
            final_path.display(),
            error
        );
    }
    Ok(false)
}

fn publish_by_placeholder(destination_temp: SiblingTemp, final_path: &Path) -> Result<()> {
    publish_by_placeholder_with(destination_temp, final_path, |_| {})
}

fn publish_by_placeholder_with(
    destination_temp: SiblingTemp,
    final_path: &Path,
    before_rename: impl FnOnce(&Path),
) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(final_path)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                return collision_state_changed_error(final_path);
            }
            destination_refused_commit_error(final_path, error)
        })?;
    before_rename(final_path);
    destination_temp
        .commit_with(final_path, |temp, destination| {
            let meta = std::fs::symlink_metadata(destination)?;
            if meta.file_type().is_symlink() || !meta.is_file() || meta.len() != 0 {
                return Err(std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "destination placeholder was modified before commit",
                ));
            }
            #[expect(
                clippy::disallowed_methods,
                reason = "installs over this job's own empty placeholder, never over a user's file"
            )]
            std::fs::rename(temp, destination)
        })
        .map_err(|error| {
            if std::fs::symlink_metadata(final_path)
                .is_ok_and(|entry| entry.is_file() && entry.len() == 0)
            {
                if let Err(cleanup) = std::fs::remove_file(final_path) {
                    log::warn!(
                        "Could not remove the empty placeholder after a refused commit: {cleanup}"
                    );
                }
            }
            destination_refused_commit_error(final_path, error)
        })
}

fn publish_without_replacing(temp_output: &Path, final_path: &Path) -> Result<PathBuf> {
    let started = Instant::now();
    if path_entry_exists(final_path)? {
        return Err(collision_state_changed_error(final_path));
    }
    flush_staged_output(temp_output)?;
    if link_without_replacing(temp_output, final_path)? {
        log::info!(
            "finalize_move method=hard-link status=ok elapsed_ms={} dest={}",
            started.elapsed().as_millis(),
            final_path.display(),
        );
        return Ok(final_path.to_path_buf());
    }
    publish_copy_without_replacing(temp_output, final_path)
}

pub(super) fn publish_copy_without_replacing(source: &Path, final_path: &Path) -> Result<PathBuf> {
    let started = Instant::now();
    let destination_temp = replacement_temp(final_path)?;
    copy_staged_output_to_new_file(source, destination_temp.path())?;
    let method = if link_without_replacing(destination_temp.path(), final_path)? {
        "copy-link"
    } else {
        publish_by_placeholder(destination_temp, final_path)?;
        "copy-placeholder-rename"
    };
    log::info!(
        "finalize_move method={method} status=ok elapsed_ms={} dest={}",
        started.elapsed().as_millis(),
        final_path.display()
    );
    Ok(final_path.to_path_buf())
}

fn replacement_temp_tag(final_path: &Path) -> Result<String> {
    let file_name = final_path.file_name().ok_or_else(|| {
        AppError::FileValidation(format!(
            "Output path '{}' has no file name.",
            sanitize_path_for_display(final_path)
        ))
    })?;
    let hash = abb_media_core::sha256_hex(file_name.as_encoded_bytes());
    Ok(format!("replace_install_{}", &hash[..16]))
}

fn replacement_temp(final_path: &Path) -> Result<SiblingTemp> {
    SiblingTemp::beside(final_path, &replacement_temp_tag(final_path)?, "").map_err(|error| {
        AppError::FileValidation(format!(
            "Output path '{}' has no parent directory: {error}",
            sanitize_path_for_display(final_path)
        ))
    })
}

pub(super) fn remove_stale_replacement_temps(final_path: &Path) {
    let (Some(parent), Ok(tag)) = (final_path.parent(), replacement_temp_tag(final_path)) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let prefix = SiblingTemp::name_prefix(&tag);
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let Some(entry_name) = entry_name.to_str() else {
            continue;
        };
        if !entry_name.starts_with(&prefix) {
            continue;
        }
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => log::info!(
                "finalize_move stale_replacement_temp=removed name={entry_name} dest_dir={}",
                sanitize_path_for_display(parent)
            ),
            Err(error) => log::warn!(
                "finalize_move stale_replacement_temp=remove_failed name={entry_name} err={error}"
            ),
        }
    }
}

fn replace_existing_from_staged_output_with<R>(
    temp_output: &Path,
    final_path: &Path,
    mut replace_file: R,
) -> Result<PathBuf>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let started = Instant::now();
    flush_staged_output(temp_output)?;
    match replace_file(temp_output, final_path) {
        Ok(()) => {
            log::info!(
                "finalize_move method=rename-replace status=ok elapsed_ms={} dest={}",
                started.elapsed().as_millis(),
                final_path.display()
            );
            return Ok(final_path.to_path_buf());
        }
        Err(rename_err) if matches!(rename_err.kind(), ErrorKind::CrossesDevices) => {
            log::info!(
                "finalize_move method=rename-replace status=cross-device-fallback dest={}",
                final_path.display()
            );
        }
        Err(rename_err) => {
            log::warn!(
                "finalize_move method=rename status=err elapsed_ms={} dest={} err={}",
                started.elapsed().as_millis(),
                final_path.display(),
                rename_err
            );
            return Err(destination_refused_commit_error(final_path, rename_err));
        }
    }

    let destination_temp = replacement_temp(final_path)?;
    copy_staged_output_to_new_file(temp_output, destination_temp.path())?;
    destination_temp
        .commit_with(final_path, &mut replace_file)
        .map_err(|error| destination_refused_commit_error(final_path, error))?;
    log::info!(
        "finalize_move method=copy-replace status=ok elapsed_ms={} dest={}",
        started.elapsed().as_millis(),
        final_path.display()
    );
    Ok(final_path.to_path_buf())
}

fn replace_existing_from_staged_output(temp_output: &Path, final_path: &Path) -> Result<PathBuf> {
    replace_existing_from_staged_output_with(temp_output, final_path, |source, destination| {
        crate::file_replace::replace_file(source, destination)
    })
}

fn commit_temp_output_to_artifact(
    temp_output: PathBuf,
    final_path: &Path,
    action: PlannedOutputAction,
) -> Result<PathBuf> {
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::FileValidation(format!(
                "Cannot create output directory '{}': {e}",
                sanitize_path_for_display(parent)
            ))
        })?;
    }
    let published = match action {
        PlannedOutputAction::Write | PlannedOutputAction::RenameNew => {
            publish_without_replacing(&temp_output, final_path)?
        }
        PlannedOutputAction::ReplaceExisting => {
            replace_existing_from_staged_output(&temp_output, final_path)?
        }
        PlannedOutputAction::SkipExisting | PlannedOutputAction::ReviewRequired => {
            return Err(AppError::FileValidation(format!(
                "Output commit is not allowed for '{}'. Review the output plan and retry.",
                sanitize_path_for_display(final_path)
            )));
        }
    };
    remove_stale_replacement_temps(final_path);
    Ok(published)
}

pub(crate) struct OutputCommitRequest<'a> {
    final_path: &'a Path,
    action: PlannedOutputAction,
}

impl<'a> OutputCommitRequest<'a> {
    pub(crate) fn new(final_path: &'a Path, action: PlannedOutputAction) -> Self {
        Self { final_path, action }
    }
}

pub(crate) struct FinalizedOutputSuccess {
    pub ui_message: &'static str,
    pub result_message: String,
}

pub(crate) fn commit_output_artifact(
    request: OutputCommitRequest<'_>,
    temp_output: PathBuf,
) -> Result<PathBuf> {
    commit_temp_output_to_artifact(temp_output, request.final_path, request.action)
}

pub(crate) fn cleanup_failure_warning(error: &AppError) -> String {
    log::warn!("Final output committed; temporary cleanup will retry on drop: {error}");
    "The output is ready, but some temporary files could not be removed.".to_string()
}

pub(crate) fn finalized_output_success(
    output_kind: OutputKind,
    final_output: &Path,
    cancelled_after_commit: bool,
    cleanup_warning: Option<&str>,
) -> FinalizedOutputSuccess {
    if cancelled_after_commit {
        log::warn!(
            "Cancellation arrived after final output commit; reporting success for {}",
            final_output.display()
        );
    }

    let mut success = match output_kind {
        OutputKind::Preview => FinalizedOutputSuccess {
            ui_message: "Preview created successfully",
            result_message: format!("Successfully created preview: {}", final_output.display()),
        },
        OutputKind::Final => FinalizedOutputSuccess {
            ui_message: "Processing complete",
            result_message: format!("Successfully created audiobook: {}", final_output.display()),
        },
    };
    if let Some(warning) = cleanup_warning {
        success.ui_message = "Output created; temporary cleanup needs attention";
        success.result_message.push_str(". ");
        success.result_message.push_str(warning);
    }
    success
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
