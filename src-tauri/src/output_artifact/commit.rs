use super::collision::path_entry_exists;
use super::types::{OutputKind, PlannedOutputAction};
use crate::audio::cleanup::CleanupGuard;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::context::ProcessingContext;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

fn collision_state_changed_error(final_path: &Path) -> AppError {
    AppError::FileValidation(format!(
        "Output collision state changed for '{}'. Review collisions and try again.",
        sanitize_path_for_display(final_path)
    ))
}

fn install_without_replacing(temp_output: &Path, final_path: &Path) -> Result<PathBuf> {
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
                "finalize_move method=hard-link status=ok dest={}",
                final_path.display()
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

    let mut source = std::fs::File::open(temp_output).map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot open temporary output '{}' for final copy: {}",
            sanitize_path_for_display(temp_output),
            error
        ))
    })?;
    let mut created_destination = false;
    let copy_result = (|| -> Result<()> {
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
        Ok(())
    })();

    if let Err(error) = copy_result {
        if created_destination {
            let _ = std::fs::remove_file(final_path);
        }
        return Err(error);
    }

    std::fs::remove_file(temp_output).map_err(|error| {
        AppError::FileValidation(format!(
            "Created final output '{}' but failed to remove temporary file: {}",
            sanitize_path_for_display(final_path),
            error
        ))
    })?;
    log::info!(
        "finalize_move method=copy-create-new status=ok dest={}",
        final_path.display()
    );
    Ok(final_path.to_path_buf())
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
    match std::fs::rename(&temp_output, final_path) {
        Ok(()) => {
            log::info!(
                "finalize_move method=rename-replace status=ok dest={}",
                final_path.display()
            );
            Ok(final_path.to_path_buf())
        }
        Err(rename_err) => {
            log::warn!(
                "finalize_move method=rename status=err dest={} err={}",
                final_path.display(),
                rename_err
            );
            Err(AppError::FileValidation(format!(
                "Cannot replace final output '{}' with staged output: {}",
                sanitize_path_for_display(final_path),
                rename_err
            )))
        }
    }
}

pub(crate) struct OutputCommitOutcome {
    pub final_output: PathBuf,
    pub cancelled: bool,
}

pub(crate) struct FinalizedOutputSuccess {
    pub ui_message: &'static str,
    pub result_message: String,
}

fn commit_output_artifact_internal<F>(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
) -> Result<OutputCommitOutcome>
where
    F: FnOnce(),
{
    let final_output =
        commit_temp_output_to_artifact(temp_output, destination, context.output.commit_action())?;

    // Destination output is now canonical and must not be cleaned up on cancellation.
    cleanup_guard.remove_path(&final_output);
    after_move();

    let cancelled = context.is_cancelled();
    cleanup_guard.cleanup_now()?;

    Ok(OutputCommitOutcome {
        final_output,
        cancelled,
    })
}

pub(crate) fn commit_output_artifact(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
) -> Result<OutputCommitOutcome> {
    commit_output_artifact_internal(context, temp_output, destination, cleanup_guard, || {})
}

#[cfg(test)]
pub(crate) fn commit_output_artifact_with_hook<F>(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
) -> Result<OutputCommitOutcome>
where
    F: FnOnce(),
{
    commit_output_artifact_internal(context, temp_output, destination, cleanup_guard, after_move)
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
mod tests {
    use super::*;
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::audio::SampleRateConfig;
    use crate::processing::context::OutputConfig;
    use crate::processing::job_registry::CancellationChecker;
    use crate::processing::session::ProcessingSession;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn test_context(final_path: &Path, session: Arc<ProcessingSession>) -> ProcessingContext {
        ProcessingContext::new_headless(
            session,
            EncoderSettings {
                encoder_type: EncoderType::NativeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Cbr,
                channels: ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            SampleRateConfig::Auto,
            OutputConfig::new(final_path),
        )
    }

    #[test]
    fn commit_output_artifact_preserves_moved_output_on_post_move_cancel() {
        let root = TempDir::new().expect("temp root");
        let temp_dir = root.path().join("worker-temp");
        std::fs::create_dir_all(&temp_dir).expect("create worker temp dir");

        let temp_output = temp_dir.join("worker-output.m4b");
        std::fs::write(&temp_output, b"payload").expect("write temp output");

        let final_output = root.path().join("final-output.m4b");

        let job_flag = Arc::new(AtomicBool::new(false));
        let global_flag = Arc::new(AtomicBool::new(false));
        let checker = CancellationChecker {
            job_flag: job_flag.clone(),
            global_flag,
        };
        let session = Arc::new(ProcessingSession::from_job_registry(
            uuid::Uuid::new_v4(),
            checker,
        ));
        let context = test_context(&final_output, session);

        let mut cleanup_guard = CleanupGuard::new(context.session.id());
        cleanup_guard.add_path(&temp_dir);
        cleanup_guard.add_path(&temp_output);

        let outcome = commit_output_artifact_with_hook(
            &context,
            temp_output,
            &final_output,
            &mut cleanup_guard,
            || {
                job_flag.store(true, Ordering::Release);
            },
        )
        .expect("commit should succeed");

        assert!(outcome.cancelled, "expected cancellation after move");
        assert!(final_output.exists(), "moved output should be preserved");
        assert!(
            !temp_dir.exists(),
            "temp directory should still be cleaned after post-move cancellation"
        );
    }

    #[test]
    fn finalized_output_success_keeps_success_messages_after_post_commit_cancel() {
        let output = Path::new("/tmp/final-output.m4b");

        let preview = finalized_output_success(OutputKind::Preview, output, true);
        assert_eq!(preview.ui_message, "Preview created successfully");
        assert_eq!(
            preview.result_message,
            "Successfully created preview: /tmp/final-output.m4b"
        );

        let full = finalized_output_success(OutputKind::Final, output, true);
        assert_eq!(full.ui_message, "Processing complete");
        assert_eq!(
            full.result_message,
            "Successfully created audiobook: /tmp/final-output.m4b"
        );
    }

    #[test]
    fn commit_temp_output_rejects_existing_destination_for_write_action() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::fs::write(&final_output, b"existing").expect("write existing output");

        let err = commit_temp_output_to_artifact(
            temp_output.clone(),
            &final_output,
            PlannedOutputAction::Write,
        )
        .expect_err("existing destination should fail");

        assert!(err.to_string().contains("Review collisions and try again"));
        assert_eq!(
            std::fs::read(&final_output).expect("read final output"),
            b"existing"
        );
        assert_eq!(
            std::fs::read(&temp_output).expect("read temp output"),
            b"new"
        );
    }

    #[test]
    fn commit_temp_output_replaces_existing_destination_for_replace_existing_action() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::fs::write(&final_output, b"existing").expect("write existing output");

        let committed = commit_temp_output_to_artifact(
            temp_output.clone(),
            &final_output,
            PlannedOutputAction::ReplaceExisting,
        )
        .expect("replace action should succeed");

        assert_eq!(committed, final_output);
        assert_eq!(
            std::fs::read(&final_output).expect("read final output"),
            b"new"
        );
        assert!(!temp_output.exists(), "temp output should be removed");
    }

    #[test]
    fn commit_temp_output_preserves_outputs_when_replace_rename_fails() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::fs::create_dir(&final_output).expect("create occupied destination directory");

        let err = commit_temp_output_to_artifact(
            temp_output.clone(),
            &final_output,
            PlannedOutputAction::ReplaceExisting,
        )
        .expect_err("rename into occupied directory should fail");

        assert!(err.to_string().contains("Cannot replace final output"));
        assert!(final_output.is_dir(), "existing destination should remain");
        assert_eq!(
            std::fs::read(&temp_output).expect("read temp output"),
            b"new",
            "staged output should remain for cleanup after commit failure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn commit_temp_output_rejects_dangling_symlink_destination_for_write_action() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        let missing_target = root.path().join("missing-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::os::unix::fs::symlink(&missing_target, &final_output)
            .expect("create dangling symlink");

        let err = commit_temp_output_to_artifact(
            temp_output.clone(),
            &final_output,
            PlannedOutputAction::Write,
        )
        .expect_err("dangling symlink should fail");

        assert!(err.to_string().contains("Review collisions and try again"));
        assert_eq!(
            std::fs::read_link(&final_output).expect("read symlink"),
            missing_target
        );
        assert_eq!(
            std::fs::read(&temp_output).expect("read temp output"),
            b"new"
        );
    }
}
