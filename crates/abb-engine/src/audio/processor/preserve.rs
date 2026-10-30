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
    let accepted = file.chapter_plan.as_ref().ok_or_else(|| {
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
    copy_with_cancellation(
        &file.path,
        &accepted.source_fingerprint,
        &staged,
        &context,
        0.0..1.0,
    )?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    let target_extension = context
        .output
        .final_path()
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| AppError::InvalidInput("Output extension is missing.".into()))?;
    crate::metadata::remux_preserved_audio_container(
        &staged,
        target_extension,
        &accepted.chapters,
    )?;
    if let Some(patch) = metadata_intent.as_ref() {
        context
            .new_emitter()
            .emit_metadata_start("Writing metadata...");
        crate::metadata::save_metadata_intent(&staged, patch)?;
    }
    crate::metadata::verify_chapters(&staged, &accepted.chapters)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    super::finalize::complete_staged_output(&context, staged, &mut cleanup)
}

pub(super) fn copy_with_cancellation(
    source: &std::path::Path,
    expected_source_fingerprint: &str,
    destination: &std::path::Path,
    context: &ProcessingContext,
    progress_range: std::ops::Range<f32>,
) -> Result<()> {
    // A queued job may reopen the source long after preflight inspected it.
    let source = crate::audio::validate_input_audio_path(source)?;
    let mut input = std::fs::File::open(source)?;
    let source_metadata = input.metadata()?;
    crate::metadata::validate_source_fingerprint(&source_metadata, expected_source_fingerprint)?;
    let total_bytes = source_metadata.len();
    let mut copied_bytes = 0_u64;
    let mut reported_percent = 0_u64;
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
            emit_progress(
                context,
                progress_range.start
                    + (progress_range.end - progress_range.start)
                        * (percent.min(100) as f32 / 100.0),
                "Preserving original audio...",
            );
        }
    }
    if copied_bytes != total_bytes {
        return Err(AppError::InvalidInput(
            "Audio changed since inspection. Remove and import it again.".to_string(),
        ));
    }
    let source_metadata = input.metadata()?;
    crate::metadata::validate_source_fingerprint(&source_metadata, expected_source_fingerprint)?;
    output.sync_all()?;
    Ok(())
}

pub(super) fn emit_progress(context: &ProcessingContext, fraction: f32, message: &str) {
    context.new_emitter().emit_converting_progress(
        crate::processing::converting_percentage(fraction),
        message,
        None,
        None,
    );
}

#[cfg(test)]
#[path = "preserve_tests.rs"]
mod tests;
