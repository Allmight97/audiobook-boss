use super::workspace::Workspace;
use super::AudioRunOutcome;
use crate::audio::{AudioFile, FileListInfo};
use crate::errors::{AppError, Result};
use crate::execution::ProcessingContext;
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy, MetadataIntentPatch};
use std::io::{Read, Write};
use std::path::PathBuf;

/// What a preserve route writes once its audio is staged.
pub(super) enum MetadataStrategy {
    /// One staged source: move it to the target container and apply the
    /// planned patch onto its own tags.
    PatchStagedCopy { patch: Option<MetadataIntentPatch> },
    /// A join of several sources: write the full effective metadata onto the
    /// new file.
    WriteFreshMux {
        metadata: Option<AudiobookMetadata>,
        cover_policy: CoverArtPassthroughPolicy,
    },
}

impl MetadataStrategy {
    fn copy_share(&self) -> f32 {
        match self {
            Self::PatchStagedCopy { .. } => 1.0,
            Self::WriteFreshMux { .. } => 0.5,
        }
    }
}

pub(super) fn execute(
    context: ProcessingContext,
    file_info: FileListInfo,
    strategy: MetadataStrategy,
) -> Result<AudioRunOutcome> {
    if matches!(strategy, MetadataStrategy::PatchStagedCopy { .. }) {
        check_single_source(&file_info)?;
    }
    let mut workspace = Workspace::open(&context)?;
    context
        .new_emitter()
        .emit_converting_start("Preserving original audio...");
    let copies = stage_copies(
        &context,
        &workspace,
        &file_info.files,
        strategy.copy_share(),
    )?;
    let (staged, warnings) = match strategy {
        MetadataStrategy::PatchStagedCopy { patch } => (
            patch_staged_copy(&context, &copies[0], patch.as_ref())?,
            Vec::new(),
        ),
        MetadataStrategy::WriteFreshMux {
            metadata,
            cover_policy,
        } => super::preserve_merge::write_fresh_mux(
            &context,
            &workspace,
            copies,
            metadata,
            cover_policy,
        )?,
    };
    Ok(AudioRunOutcome {
        message: super::finalize::complete_staged_output(&context, staged, &mut workspace)?,
        warnings,
    })
}

fn missing_source_identity() -> AppError {
    AppError::InvalidInput(
        "Preserve audio requires inspected source identity. Remove and import it again."
            .to_string(),
    )
}

fn check_single_source(file_info: &FileListInfo) -> Result<()> {
    let file = file_info.files.first().ok_or_else(|| {
        AppError::InvalidInput("Preserve audio requires one inspected input.".to_string())
    })?;
    crate::audio::validate_preservation_source(file)?;
    if file_info.files.len() != 1 {
        return Err(AppError::InvalidInput(
            "Preserve audio cannot merge multiple inputs.".to_string(),
        ));
    }
    if file.chapter_plan.is_none() {
        return Err(missing_source_identity());
    }
    Ok(())
}

fn stage_copies(
    context: &ProcessingContext,
    workspace: &Workspace,
    files: &[AudioFile],
    share: f32,
) -> Result<Vec<AudioFile>> {
    let mut copies = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let fingerprint = file
            .chapter_plan
            .as_ref()
            .ok_or_else(missing_source_identity)?
            .source_fingerprint
            .as_str();
        let folder = workspace.dir().join(format!("source-{index}"));
        std::fs::create_dir_all(&folder).map_err(AppError::Io)?;
        let path = folder.join(file.path.file_name().unwrap_or_default());
        copy_with_cancellation(
            &file.path,
            fingerprint,
            &path,
            context,
            (share * index as f32 / files.len() as f32)
                ..(share * (index + 1) as f32 / files.len() as f32),
        )?;
        let mut copy = file.clone();
        copy.path = path;
        copies.push(copy);
    }
    Ok(copies)
}

fn patch_staged_copy(
    context: &ProcessingContext,
    copy: &AudioFile,
    patch: Option<&MetadataIntentPatch>,
) -> Result<PathBuf> {
    let accepted = copy
        .chapter_plan
        .as_ref()
        .ok_or_else(missing_source_identity)?;
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
        &copy.path,
        target_extension,
        &accepted.chapters,
    )?;
    if let Some(patch) = patch {
        context
            .new_emitter()
            .emit_metadata_start("Writing metadata...");
        crate::metadata::save_metadata_intent(&copy.path, patch)?;
    }
    crate::metadata::verify_chapters(&copy.path, &accepted.chapters)?;
    if context.is_cancelled() {
        return Err(AppError::cancelled());
    }
    Ok(copy.path.clone())
}

pub(super) fn copy_with_cancellation(
    source: &std::path::Path,
    expected_source_fingerprint: &str,
    destination: &std::path::Path,
    context: &ProcessingContext,
    progress_range: std::ops::Range<f32>,
) -> Result<()> {
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
        crate::execution::converting_percentage(fraction),
        message,
        None,
        None,
    );
}

#[cfg(test)]
#[path = "preserve_tests.rs"]
mod tests;
