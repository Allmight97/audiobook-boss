use crate::audio::CleanupGuard;
use crate::audio::FileListInfo;
use crate::errors::{AppError, Result};
use crate::metadata::MetadataIntentPatch;
use crate::processing::ProcessingContext;
use std::io::{Read, Write};

pub(super) fn execute_preserved_audio(
    context: ProcessingContext,
    file_info: FileListInfo,
    metadata_intent: Option<MetadataIntentPatch>,
) -> Result<String> {
    if context.preview.is_some() {
        return Err(AppError::InvalidInput(
            "Preserve audio cannot be used for a preview.".to_string(),
        ));
    }
    let file = file_info.files.first().ok_or_else(|| {
        AppError::InvalidInput("Preserve audio requires one inspected input.".to_string())
    })?;
    crate::audio::validate_preservation_source(file)?;
    if file_info.files.len() != 1 {
        return Err(AppError::InvalidInput(
            "Preserve audio cannot merge multiple inputs.".to_string(),
        ));
    }
    let source_fingerprint = file
        .chapter_plan
        .as_ref()
        .map(|plan| plan.source_fingerprint.as_str())
        .ok_or_else(|| {
            AppError::InvalidInput(
                "Preserve audio requires inspected source identity. Remove and import it again."
                    .to_string(),
            )
        })?;

    let workspace = super::staging::create_processing_workspace_dir(
        context.session.uuid(),
        context.processing_workspace_root(),
    )?;
    let mut cleanup = CleanupGuard::new(context.session.id());
    cleanup.add_path(&workspace);
    let extension = file
        .path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AppError::InvalidInput("Preserve source has no supported extension.".to_string())
        })?;
    let staged = workspace.join(format!("preserved.{extension}"));
    cleanup.add_path(&staged);
    context
        .new_emitter()
        .emit_converting_start("Preserving original audio...");
    copy_with_cancellation(&file.path, source_fingerprint, &staged, &context)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    if let Some(patch) = metadata_intent.as_ref() {
        context
            .new_emitter()
            .emit_metadata_start("Writing metadata...");
        crate::metadata::save_metadata_intent(&staged, patch)?;
    }
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    super::finalize::complete_staged_output(&context, staged, &mut cleanup)
}

fn copy_with_cancellation(
    source: &std::path::Path,
    expected_source_fingerprint: &str,
    destination: &std::path::Path,
    context: &ProcessingContext,
) -> Result<()> {
    // A queued job may reopen the source long after preflight inspected it.
    let source = crate::audio::validate_input_audio_path(source)?;
    let mut input = std::fs::File::open(source)?;
    let source_metadata = input.metadata()?;
    crate::metadata::validate_source_fingerprint(&source_metadata, expected_source_fingerprint)?;
    let total_bytes = source_metadata.len();
    let mut copied_bytes = 0_u64;
    let mut reported_percent = 0_u64;
    let progress = context.new_emitter();
    let mut output = std::fs::File::create(destination)?;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        if context.is_cancelled() {
            return Err(AppError::cancelled());
        }
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        copied_bytes += count as u64;
        let percent = copied_bytes.saturating_mul(100) / total_bytes.max(1);
        if percent > reported_percent {
            reported_percent = percent;
            progress.emit_converting_progress(
                crate::processing::progress::PROGRESS_CONVERTING_START
                    + (crate::processing::progress::PROGRESS_CONVERTING_MAX
                        - crate::processing::progress::PROGRESS_CONVERTING_START)
                        * (percent.min(100) as f32 / 100.0),
                "Preserving original audio...",
                None,
                None,
            );
        }
    }
    output.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::{JobRegistry, OutputConfig, ProcessingSession};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn inspect_source_fingerprint(path: &std::path::Path) -> String {
        crate::metadata::inspect_chapter_source(path, 1_000, &[])
            .expect("inspect source identity")
            .0
            .source_fingerprint
    }

    #[cfg(unix)]
    #[test]
    fn preserve_copy_rejects_a_source_replaced_with_a_symlink_after_inspection() {
        let tmp = tempfile::tempdir().expect("copy workspace");
        let source = tmp.path().join("source.mp3");
        let replacement = tmp.path().join("replacement.mp3");
        let staged = tmp.path().join("staged.mp3");
        std::fs::write(&source, b"inspected source").expect("write source");
        let inspected_path = crate::audio::validate_input_audio_path(&source)
            .expect("source accepted before scheduler wait");
        let source_fingerprint = inspect_source_fingerprint(&inspected_path);
        std::fs::write(&replacement, b"replacement audio").expect("write replacement");
        std::fs::remove_file(&source).expect("remove old source");
        std::os::unix::fs::symlink(&replacement, &source).expect("replace source with symlink");
        let context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            None,
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(tmp.path().join("final.mp3")),
        );
        let error = copy_with_cancellation(&inspected_path, &source_fingerprint, &staged, &context)
            .expect_err("queued replacement must be rejected before copying");
        assert!(matches!(error, AppError::InvalidInput(message) if message.contains("Symlinks")));
        assert!(!staged.exists());
        assert_eq!(
            std::fs::read(replacement).expect("unchanged replacement"),
            b"replacement audio"
        );
    }

    #[test]
    fn preserve_copy_rejects_a_regular_source_replaced_after_inspection() {
        let tmp = tempfile::tempdir().expect("copy workspace");
        let source = tmp.path().join("source.mp3");
        let replacement = tmp.path().join("replacement.mp3");
        let staged = tmp.path().join("staged.mp3");
        std::fs::write(&source, b"inspected source").expect("write source");
        let inspected_path = crate::audio::validate_input_audio_path(&source)
            .expect("source accepted before scheduler wait");
        let source_fingerprint = inspect_source_fingerprint(&inspected_path);
        std::fs::write(&replacement, b"different replacement audio").expect("write replacement");
        std::fs::remove_file(&source).expect("remove old source");
        std::fs::rename(&replacement, &source).expect("install regular replacement");
        let context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            None,
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(tmp.path().join("final.mp3")),
        );

        let error = copy_with_cancellation(&inspected_path, &source_fingerprint, &staged, &context)
            .expect_err("queued regular-file replacement must be rejected before copying");

        assert!(
            matches!(error, AppError::InvalidInput(message) if message.contains("changed since"))
        );
        assert!(!staged.exists());
        assert_eq!(
            std::fs::read(source).expect("unchanged replacement"),
            b"different replacement audio"
        );
    }

    #[tokio::test]
    async fn preserve_copy_stops_after_cancellation_during_progress() {
        let tmp = tempfile::tempdir().expect("copy workspace");
        let source = tmp.path().join("source.mp3");
        let staged = tmp.path().join("staged.mp3");
        std::fs::write(&source, vec![7_u8; 4 * 1024 * 1024]).expect("write copy source");
        let source_fingerprint = inspect_source_fingerprint(&source);
        let registry = JobRegistry::new(1);
        let (job_id, _permit) = registry.register_job().await.expect("register copy job");
        let cancelled = Arc::new(AtomicBool::new(false));
        let checker = registry
            .cancellation_checker(job_id)
            .await
            .with_operation_flag(Some(cancelled.clone()));
        let mut context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::from_job_registry(job_id.0, checker)),
            None,
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(tmp.path().join("final.mp3")),
        );
        context.progress_listener = Some(Arc::new(move |_| {
            cancelled.store(true, Ordering::Release);
        }));
        let error = copy_with_cancellation(&source, &source_fingerprint, &staged, &context)
            .expect_err("copy must observe cancellation");
        assert!(matches!(error, AppError::Cancellation(_)));
        let copied_bytes = std::fs::metadata(&staged).expect("partial copy").len();
        assert!(copied_bytes > 0 && copied_bytes < 4 * 1024 * 1024);
        assert_eq!(
            std::fs::metadata(&source).expect("original source").len(),
            4 * 1024 * 1024
        );
    }
}
