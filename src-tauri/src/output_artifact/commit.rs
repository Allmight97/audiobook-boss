use super::collision::path_entry_exists;
use super::types::{OutputKind, PlannedOutputAction};
use crate::audio::CleanupGuard;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn collision_state_changed_error(final_path: &Path) -> AppError {
    AppError::FileValidation(format!(
        "Output collision state changed for '{}'. Review collisions and try again.",
        sanitize_path_for_display(final_path)
    ))
}

fn remove_copied_temp_output_with<H, R>(
    temp_output: &Path,
    final_path: &Path,
    source_handle: H,
    remove_file: R,
) -> Result<()>
where
    R: FnOnce(&Path) -> std::io::Result<()>,
{
    // SMB/NAS mounts can reject deleting a file while ABB still holds a read
    // handle. Close the copied source before removing the staged output.
    drop(source_handle);
    remove_file(temp_output).map_err(|error| {
        AppError::FileValidation(format!(
            "Created final output '{}' but failed to remove temporary file: {}",
            sanitize_path_for_display(final_path),
            error
        ))
    })
}

fn remove_copied_temp_output<H>(
    temp_output: &Path,
    final_path: &Path,
    source_handle: H,
) -> Result<()> {
    remove_copied_temp_output_with(temp_output, final_path, source_handle, |path| {
        std::fs::remove_file(path)
    })
}

fn copy_staged_output_to_new_file(temp_output: &Path, destination_path: &Path) -> Result<()> {
    let mut source = std::fs::File::open(temp_output).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot open temporary output '{}' for final copy: {}",
            sanitize_path_for_display(temp_output),
            error
        ))
    })?;
    let mut destination = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination_path)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                return collision_state_changed_error(destination_path);
            }
            AppError::FileValidation(format!(
                "Cannot create final output '{}': {}",
                sanitize_path_for_display(destination_path),
                error
            ))
        })?;
    std::io::copy(&mut source, &mut destination).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot copy file to final location '{}': {}",
            sanitize_path_for_display(destination_path),
            error
        ))
    })?;
    destination.sync_all().map_err(|error| {
        AppError::FileValidation(format!(
            "Failed to flush final output '{}': {}",
            sanitize_path_for_display(destination_path),
            error
        ))
    })?;
    drop(source);
    Ok(())
}

fn install_without_replacing(temp_output: &Path, final_path: &Path) -> Result<PathBuf> {
    let started = Instant::now();
    if path_entry_exists(final_path)? {
        return Err(collision_state_changed_error(final_path));
    }

    match std::fs::hard_link(temp_output, final_path) {
        Ok(()) => {
            std::fs::remove_file(temp_output).map_err(|error| {
                AppError::FileValidation(format!(
                    "Created final output '{}' but failed to remove temporary file: {}",
                    sanitize_path_for_display(final_path),
                    error
                ))
            })?;
            log::info!(
                "finalize_move method=hard-link status=ok elapsed_ms={} dest={}",
                started.elapsed().as_millis(),
                final_path.display(),
            );
            return Ok(final_path.to_path_buf());
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::AlreadyExists | ErrorKind::PermissionDenied
            ) && path_entry_exists(final_path)? =>
        {
            return Err(collision_state_changed_error(final_path));
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::CrossesDevices | ErrorKind::PermissionDenied | ErrorKind::Unsupported
            ) => {}
        Err(error) => {
            log::warn!(
                "finalize_move method=hard-link status=err dest={} err={}",
                final_path.display(),
                error
            );
        }
    }

    let mut created_destination = false;
    let copy_result = (|| -> Result<()> {
        let mut source = std::fs::File::open(temp_output).map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot open temporary output '{}' for final copy: {}",
                sanitize_path_for_display(temp_output),
                error
            ))
        })?;
        let mut destination = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(final_path)
            .map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    return collision_state_changed_error(final_path);
                }
                AppError::FileValidation(format!(
                    "Cannot create final output '{}': {}",
                    sanitize_path_for_display(final_path),
                    error
                ))
            })?;
        created_destination = true;
        std::io::copy(&mut source, &mut destination).map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot copy file to final location '{}': {}",
                sanitize_path_for_display(final_path),
                error
            ))
        })?;
        destination.sync_all().map_err(|error| {
            AppError::FileValidation(format!(
                "Failed to flush final output '{}': {}",
                sanitize_path_for_display(final_path),
                error
            ))
        })?;
        remove_copied_temp_output(temp_output, final_path, source)?;
        Ok(())
    })();

    if let Err(error) = copy_result {
        if created_destination {
            let _ = std::fs::remove_file(final_path);
        }
        return Err(error);
    }

    log::info!(
        "finalize_move method=copy-create-new status=ok elapsed_ms={} dest={}",
        started.elapsed().as_millis(),
        final_path.display()
    );
    Ok(final_path.to_path_buf())
}

fn destination_replacement_temp_path(final_path: &Path) -> Result<PathBuf> {
    let parent = final_path.parent().ok_or_else(|| {
        AppError::FileValidation(format!(
            "Output path '{}' has no parent directory.",
            sanitize_path_for_display(final_path)
        ))
    })?;
    let file_name = final_path.file_name().ok_or_else(|| {
        AppError::FileValidation(format!(
            "Output path '{}' has no file name.",
            sanitize_path_for_display(final_path)
        ))
    })?;
    Ok(parent.join(format!(
        ".abb_replace_install_{}_{}",
        uuid::Uuid::new_v4(),
        file_name.to_string_lossy()
    )))
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
            return Err(AppError::FileValidation(format!(
                "Cannot replace final output '{}' with staged output: {}",
                sanitize_path_for_display(final_path),
                rename_err
            )));
        }
    }

    let destination_temp = destination_replacement_temp_path(final_path)?;
    if let Err(error) = copy_staged_output_to_new_file(temp_output, &destination_temp) {
        let _ = std::fs::remove_file(&destination_temp);
        return Err(error);
    }

    match replace_file(&destination_temp, final_path) {
        Ok(()) => {
            std::fs::remove_file(temp_output).map_err(|error| {
                AppError::FileValidation(format!(
                    "Created final output '{}' but failed to remove temporary file: {}",
                    sanitize_path_for_display(final_path),
                    error
                ))
            })?;
            log::info!(
                "finalize_move method=copy-replace status=ok elapsed_ms={} dest={}",
                started.elapsed().as_millis(),
                final_path.display()
            );
            Ok(final_path.to_path_buf())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&destination_temp);
            Err(AppError::FileValidation(format!(
                "Cannot replace final output '{}' with staged output: {}",
                sanitize_path_for_display(final_path),
                error
            )))
        }
    }
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
    match action {
        PlannedOutputAction::Write | PlannedOutputAction::RenameNew => {
            return install_without_replacing(&temp_output, final_path);
        }
        PlannedOutputAction::ReplaceExisting => {}
        PlannedOutputAction::SkipExisting | PlannedOutputAction::ReviewRequired => {
            return Err(AppError::FileValidation(format!(
                "Output commit is not allowed for '{}'. Review the output plan and retry.",
                sanitize_path_for_display(final_path)
            )));
        }
    }
    replace_existing_from_staged_output(&temp_output, final_path)
}

pub(crate) struct OutputCommitOutcome {
    pub final_output: PathBuf,
    pub cancelled: bool,
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

fn commit_output_artifact_internal<F, C>(
    request: OutputCommitRequest<'_>,
    temp_output: PathBuf,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
    is_cancelled: C,
) -> Result<OutputCommitOutcome>
where
    F: FnOnce(),
    C: FnOnce() -> bool,
{
    let final_output =
        commit_temp_output_to_artifact(temp_output, request.final_path, request.action)?;

    // Destination output is now canonical and must not be cleaned up on cancellation.
    cleanup_guard.remove_path(&final_output);
    after_move();

    let cancelled = is_cancelled();
    cleanup_guard.cleanup_now()?;

    Ok(OutputCommitOutcome {
        final_output,
        cancelled,
    })
}

pub(crate) fn commit_output_artifact<C>(
    request: OutputCommitRequest<'_>,
    temp_output: PathBuf,
    cleanup_guard: &mut CleanupGuard,
    is_cancelled: C,
) -> Result<OutputCommitOutcome>
where
    C: FnOnce() -> bool,
{
    commit_output_artifact_internal(request, temp_output, cleanup_guard, || {}, is_cancelled)
}

#[cfg(test)]
pub(crate) fn commit_output_artifact_after_move<F, C>(
    request: OutputCommitRequest<'_>,
    temp_output: PathBuf,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
    is_cancelled: C,
) -> Result<OutputCommitOutcome>
where
    F: FnOnce(),
    C: FnOnce() -> bool,
{
    commit_output_artifact_internal(
        request,
        temp_output,
        cleanup_guard,
        after_move,
        is_cancelled,
    )
}

pub(crate) fn finalized_output_success(
    output_kind: OutputKind,
    final_output: &Path,
    cancelled_after_commit: bool,
) -> FinalizedOutputSuccess {
    if cancelled_after_commit {
        log::warn!(
            "Cancellation arrived after final output commit; reporting success for {}",
            final_output.display()
        );
    }

    match output_kind {
        OutputKind::Preview => FinalizedOutputSuccess {
            ui_message: "Preview created successfully",
            result_message: format!("Successfully created preview: {}", final_output.display()),
        },
        OutputKind::Final => FinalizedOutputSuccess {
            ui_message: "Processing complete",
            result_message: format!("Successfully created audiobook: {}", final_output.display()),
        },
    }
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
