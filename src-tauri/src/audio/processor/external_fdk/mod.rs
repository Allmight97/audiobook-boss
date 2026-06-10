mod args;
mod passthrough;
mod process;
mod progress;

use crate::audio::settings_encoder::EncoderType;
use crate::audio::toolchain::validate_external_input_decoders;
use crate::audio::toolchain::ValidatedExternalToolchain;
use crate::audio::CleanupGuard;
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::merge_passthrough_cover_art;
use crate::metadata::{rewrite_metadata_with_ffmpeg, AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::ProcessingContext;
use std::path::PathBuf;
use std::time::Instant;

pub(super) async fn process_audiobook_with_external_fdk(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    selected_decoders: Vec<Option<DecoderSelection>>,
    metadata: Option<AudiobookMetadata>,
    cover_art_passthrough: CoverArtPassthroughPolicy,
    toolchain: ValidatedExternalToolchain,
) -> Result<String> {
    if !matches!(
        context.encoder_settings.encoder_type,
        EncoderType::Auto | EncoderType::FdkHeAac
    ) {
        return Err(AppError::InvalidInput(
            "External FDK worker only supports Auto or FDK AAC encoder selection.".to_string(),
        ));
    }

    if files.len() != selected_decoders.len() {
        return Err(AppError::General(
            "External FDK input decoder selections drifted from the file list.".to_string(),
        ));
    }

    let mut valid_files = Vec::new();
    let mut valid_selected_decoders = Vec::new();
    for (file, selection) in files.into_iter().zip(selected_decoders) {
        if file.is_valid {
            valid_files.push(file);
            valid_selected_decoders.push(selection);
        }
    }
    if valid_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No valid audio files found for external FDK processing.".to_string(),
        ));
    }
    log::info!(
        "external FDK toolchain: source={:?} path={}",
        toolchain.source,
        sanitize_path_for_display(&toolchain.ffmpeg_path)
    );
    validate_external_input_decoders(&valid_files, &valid_selected_decoders, &toolchain)?;

    let passthrough = cover_art_passthrough.apply_to_passthrough(
        passthrough::collect_passthrough_metadata(&valid_files, context.preview.is_some()),
    );

    let effective_metadata = merge_passthrough_cover_art(metadata, passthrough.as_ref());
    let ui = context.new_emitter();
    ui.emit_analyzing_start("Preparing external FDK job...");
    ui.emit_analyzing_end("External FDK toolchain validated.");
    ui.emit_converting_start("Encoding with external FDK AAC...");

    let temp_dir = create_temp_dir(&context)?;
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&temp_dir);
    let temp_output = temp_dir.join("worker-output.m4b");
    cleanup_guard.add_path(&temp_output);
    let total_duration = expected_duration_seconds(&valid_files, context.preview.as_ref());

    process::run_external_ffmpeg(
        &context,
        &ui,
        &toolchain,
        &valid_files,
        &valid_selected_decoders,
        &temp_output,
        total_duration,
    )
    .await?;

    if effective_metadata.is_some() || passthrough.is_some() {
        ui.emit_metadata_start("Re-applying metadata and cover art...");
        let metadata_started = Instant::now();
        rewrite_metadata_with_ffmpeg(
            &temp_output,
            effective_metadata.as_ref(),
            passthrough.as_ref(),
        )?;
        log::info!(
            "external_fdk_metadata_rewrite status=ok elapsed_ms={}",
            metadata_started.elapsed().as_millis()
        );
        ui.emit_finalizing("Finalizing metadata...");
    }

    super::finalize::complete_staged_output(&context, temp_output, &mut cleanup_guard, None)
}

fn create_temp_dir(context: &ProcessingContext) -> Result<PathBuf> {
    crate::audio::processor::staging::create_processing_workspace_dir(
        context.session.uuid(),
        context.processing_workspace_root(),
    )
}

fn expected_duration_seconds(
    files: &[AudioFile],
    preview: Option<&crate::processing::preview_config::PreviewConfig>,
) -> f64 {
    if let Some(preview) = preview {
        return preview.per_file_seconds(files.len()) * files.len() as f64;
    }

    let total: f64 = files.iter().filter_map(|file| file.duration).sum();
    total.max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig, ThreadSetting,
    };
    use crate::processing::{OutputConfig, ProcessingContext, ProcessingSession};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Auto,
            afterburner: false,
            threads: ThreadSetting::Auto,
            twoloop: true,
        }
    }

    #[test]
    fn external_fdk_temp_dir_uses_processing_workspace_root() {
        let local = TempDir::new().expect("local workspace");
        let destination = TempDir::new().expect("destination");
        let workspace_root = local.path().join("processing").join("sessions");
        let final_output = destination.path().join("Book.m4b");
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            encoder_settings(),
            SampleRateConfig::Auto,
            OutputConfig::new(&final_output),
            workspace_root.clone(),
        );

        let temp_dir = create_temp_dir(&context).expect("temp dir");

        assert!(temp_dir.starts_with(&workspace_root));
        assert!(!temp_dir.starts_with(destination.path()));
    }
}
