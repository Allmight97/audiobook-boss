use super::collision::path_entry_exists;
use super::types::{OutputKind, PlannedOutputAction};
use crate::audio::CleanupGuard;
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

/// User-facing wording for the destination filesystem refusing the final
/// commit (#299 Branch A): distinguishes "the destination said no" from
/// generic processing failure and names the realistic causes. The existing
/// destination file, if any, is never touched by a refused commit.
fn destination_refused_commit_error(final_path: &Path, error: impl std::fmt::Display) -> AppError {
    AppError::FileValidation(format!(
        "The destination refused the final file commit for '{}': {error}. \
         If the output folder is a network share, external drive, or \
         cloud-synced folder, check write permissions and free space, or \
         retry with a local output folder.",
        sanitize_path_for_display(final_path),
    ))
}

/// Copies the staged output into a new file at `destination_path` and flushes
/// it. Callers pass a fresh `SiblingTemp` path, whose guard removes a failed copy.
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

/// Links `source` at `final_path`, which fails rather than replace an entry
/// there. `Ok(false)`: publish by copy instead.
fn link_without_replacing(source: &Path, final_path: &Path) -> Result<bool> {
    match std::fs::hard_link(source, final_path) {
        Ok(()) => Ok(true),
        Err(error) => classify_link_failure(&error, final_path),
    }
}

/// A taken name is a collision. Any other failure goes to the copy paths,
/// which never replace an entry, because network and FUSE disks report a
/// missing hard-link feature with many different errors.
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

/// Installs a fully copied and flushed temp at a free `final_path` on a disk
/// without hard links: an empty placeholder claims the name (refused if taken),
/// then the temp is renamed over that placeholder.
fn publish_by_placeholder(destination_temp: SiblingTemp, final_path: &Path) -> Result<()> {
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
    // shortcut: a crash between the placeholder and the rename leaves an empty file under the final name; upgrade to a no-replace rename (renameat2, renamex_np) if that is seen.
    destination_temp
        .commit_with(final_path, |temp, destination| {
            std::fs::rename(temp, destination)
        })
        .map_err(|error| {
            if std::fs::symlink_metadata(final_path)
                .is_ok_and(|entry| entry.is_file() && entry.len() == 0)
            {
                let _ = std::fs::remove_file(final_path);
            }
            destination_refused_commit_error(final_path, error)
        })
}

/// Publishes `temp_output` under `final_path` without replacing anything
/// there. On another disk the bytes are copied and flushed beside the final
/// name first, so the name appears only with complete content.
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

/// Copies `source` into a flushed temp beside `final_path`, then links it
/// there without replacing anything. `source` never shares the published file.
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

/// `replace_install_<key>`: the key is the final name's hash, so a sweep can
/// find this file's stale temps by name without matching another file's.
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

/// Removes stale `.abb_replace_install_<key>_<uuid>` files left in the
/// destination directory by a hard crash mid-commit (#391 residual). Error
/// paths already clean these; only process death between the copy and the
/// final link or replace can strand one. ABB cannot sweep user-chosen output
/// directories at startup, so the sweep runs when ABB is about to commit the
/// same final artifact again — it matches only this file name's key, touches
/// regular files only, and never blocks the commit.
pub(super) fn remove_stale_replacement_temps(final_path: &Path) {
    let (Some(parent), Ok(tag)) = (final_path.parent(), replacement_temp_tag(final_path)) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let prefix = format!(".abb_{tag}_");
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let Some(entry_name) = entry_name.to_str() else {
            continue;
        };
        if !entry_name.starts_with(&prefix) {
            continue;
        }
        // DirEntry::file_type does not follow symlinks: links and directories
        // that merely imitate the temp naming are preserved.
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
    remove_stale_replacement_temps(final_path);
    match action {
        PlannedOutputAction::Write | PlannedOutputAction::RenameNew => {
            return publish_without_replacing(&temp_output, final_path);
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
    pub cleanup_warning: Option<String>,
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
        commit_temp_output_to_artifact(temp_output.clone(), request.final_path, request.action)?;
    cleanup_guard.add_path(temp_output);

    // Destination output is now canonical and must not be cleaned up on cancellation.
    cleanup_guard.remove_path(&final_output);
    after_move();

    let cancelled = is_cancelled();
    let cleanup_warning = cleanup_guard.cleanup_now().err().map(|error| {
        log::warn!("Final output committed; temporary cleanup will retry on drop: {error}");
        "The output is ready, but some temporary files could not be removed.".to_string()
    });

    Ok(OutputCommitOutcome {
        final_output,
        cancelled,
        cleanup_warning,
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
