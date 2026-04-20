//! Finalization stage — metadata writing, output move, and cleanup.
//!
//! All filesystem side-effects (rename/move + cleanup) are centralized here.
//! Metadata is written during mux, so `write_metadata_stage` is a no-op.
//! Cancellation checks run after each sub-step to avoid unnecessary work.

// Imports
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::audio::context::ProcessingContext;
use crate::audio::output_path::{path_entry_exists, OutputKind, PlannedOutputAction};
use crate::audio::{ProcessingStage, ProgressReporter};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::AudiobookMetadata;

use super::ProcessingWorkflow;

// Note: cover art detection helper removed; metadata writes handled during mux

// NOTE: File movement logic uses filesystem operations rather than FFmpeg output paths.
// This approach ensures compatibility across different output destinations and provides
// atomic completion semantics for the processing pipeline.
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

/// Moves temporary output to final location (filesystem boundary)
pub(crate) fn move_to_final_location(
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
                "finalize_move method=rename status=ok dest={}",
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
            // FALLBACK[FB-012]: trigger=atomic rename fails (cross-volume/permissions edge)
            // observe=warn rename failure + info copy-replace success logs
            // sunset=2026-06-30 issue=#195
            // Fallback: copy then remove temp (handles cross-volume or other rename failures)
            if path_entry_exists(final_path)? {
                if action != PlannedOutputAction::ReplaceExisting {
                    return Err(collision_state_changed_error(final_path));
                }
                if let Err(e) = std::fs::remove_file(final_path) {
                    log::warn!("finalize_move overwrite remove failed: {}", e);
                }
            }
            std::fs::copy(&temp_output, final_path).map_err(|e| {
                AppError::FileValidation(format!(
                    "Cannot copy file to final location '{}': {e}",
                    sanitize_path_for_display(final_path)
                ))
            })?;
            if let Err(e) = std::fs::remove_file(&temp_output) {
                log::warn!("finalize_move temp removal failed: {}", e);
            }
            log::info!(
                "finalize_move method=copy-replace status=ok dest={}",
                final_path.display()
            );
            Ok(final_path.to_path_buf())
        }
    }
}

pub(crate) struct FinalizeCommitOutcome {
    pub final_output: PathBuf,
    pub cancelled: bool,
}

fn commit_output_boundary_internal<F>(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
) -> Result<FinalizeCommitOutcome>
where
    F: FnOnce(),
{
    let final_output =
        move_to_final_location(temp_output, destination, context.output.commit_action())?;

    // Destination output is now canonical and must not be cleaned up on cancellation.
    cleanup_guard.remove_path(&final_output);
    after_move();

    let cancelled = context.is_cancelled();
    cleanup_guard.cleanup_now()?;

    Ok(FinalizeCommitOutcome {
        final_output,
        cancelled,
    })
}

pub(crate) fn commit_output_boundary(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
) -> Result<FinalizeCommitOutcome> {
    commit_output_boundary_internal(context, temp_output, destination, cleanup_guard, || {})
}

#[cfg(test)]
pub(crate) fn commit_output_boundary_with_hook<F>(
    context: &ProcessingContext,
    temp_output: PathBuf,
    destination: &Path,
    cleanup_guard: &mut CleanupGuard,
    after_move: F,
) -> Result<FinalizeCommitOutcome>
where
    F: FnOnce(),
{
    commit_output_boundary_internal(context, temp_output, destination, cleanup_guard, after_move)
}

/// Writes metadata if provided (UI emission included)
pub(crate) fn write_metadata_stage(
    _context: &ProcessingContext,
    merged_output: &Path,
    metadata: Option<AudiobookMetadata>,
    _passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<()> {
    let Some(metadata) = metadata else {
        log::debug!("Finalize metadata stage skipped; no metadata provided");
        return Ok(());
    };

    if !crate::metadata::mp4ameta_bridge::is_mp4_container(merged_output) {
        log::debug!("Finalize metadata stage skipped; ffmpeg-next handled metadata during mux");
        return Ok(());
    }

    let ui = _context.new_emitter();
    reporter.set_stage(ProcessingStage::WritingMetadata);
    ui.emit_metadata_start("Writing metadata...");
    crate::metadata::mp4ameta_bridge::write_metadata(merged_output, &metadata)?;
    ui.emit_finalizing("Finalizing...");
    Ok(())
}

/// Completes processing: move to final path + cleanup + final UI emit
pub(crate) fn complete_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    log::info!("🚀 Starting complete_processing stage");
    log::info!("Temporary file: {}", merged_output.display());
    log::info!(
        "Final output path: {}",
        context.output.artifact_path().display()
    );

    let ui = context.new_emitter();
    ui.emit_cleanup("Cleaning up...");

    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(workflow.temp_dir);
    let outcome = commit_output_boundary(
        context,
        merged_output,
        context.output.artifact_path(),
        &mut cleanup_guard,
    )?;
    log::info!(
        "✓ File moved successfully to: {}",
        outcome.final_output.display()
    );

    if outcome.cancelled {
        log::warn!("Processing was cancelled during completion");
        ui.emit_cancelled("Processing was cancelled");
        return Err(AppError::cancelled());
    }

    reporter.complete();
    let success_message = if context.output.output_kind() == OutputKind::Preview {
        ui.emit_complete("Preview created successfully");
        format!(
            "Successfully created preview: {}",
            outcome.final_output.display()
        )
    } else {
        ui.emit_complete("Processing complete");
        format!(
            "Successfully created audiobook: {}",
            outcome.final_output.display()
        )
    };
    log::info!("🎉 {}", success_message);
    Ok(success_message)
}

/// Finalize pipeline: metadata + completion
pub(crate) async fn finalize_processing(
    context: &ProcessingContext,
    workflow: ProcessingWorkflow,
    merged_output: PathBuf,
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<crate::metadata::passthrough::PassthroughMetadata>,
    reporter: &mut ProgressReporter,
) -> Result<String> {
    let passthrough_ref = passthrough.as_ref();
    write_metadata_stage(context, &merged_output, metadata, passthrough_ref, reporter)?;

    complete_processing(context, workflow, merged_output, reporter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::cleanup::CleanupGuard;
    use crate::audio::context::OutputConfig;
    use crate::audio::job_registry::CancellationChecker;
    use crate::audio::session::ProcessingSession;
    use crate::audio::settings_encoder::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
    };
    use crate::audio::SampleRateConfig;
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
    fn commit_output_boundary_preserves_moved_output_on_post_move_cancel() {
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

        let outcome = commit_output_boundary_with_hook(
            &context,
            temp_output,
            &final_output,
            &mut cleanup_guard,
            || {
                job_flag.store(true, Ordering::Release);
            },
        )
        .expect("commit boundary should succeed");

        assert!(outcome.cancelled, "expected cancellation after move");
        assert!(final_output.exists(), "moved output should be preserved");
        assert!(
            !temp_dir.exists(),
            "temp directory should still be cleaned after post-move cancellation"
        );
    }

    #[test]
    fn move_to_final_location_rejects_existing_destination_for_write_action() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::fs::write(&final_output, b"existing").expect("write existing output");

        let err = move_to_final_location(
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

    #[cfg(unix)]
    #[test]
    fn move_to_final_location_rejects_dangling_symlink_destination_for_write_action() {
        let root = TempDir::new().expect("temp root");
        let temp_output = root.path().join("temp-output.m4b");
        let final_output = root.path().join("final-output.m4b");
        let missing_target = root.path().join("missing-output.m4b");
        std::fs::write(&temp_output, b"new").expect("write temp output");
        std::os::unix::fs::symlink(&missing_target, &final_output)
            .expect("create dangling symlink");

        let err = move_to_final_location(
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
