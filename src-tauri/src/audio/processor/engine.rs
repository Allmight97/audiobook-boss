//! ffmpeg-next engine implementation (native in-process path)

use std::path::Path;
use std::sync::Once;

use ffmpeg_next as ff;

use super::run_diagnostics::{
    append_run_record, encoding_log_enabled, write_common_run_fields, write_run_header,
    write_run_identity,
};
use crate::audio::cleanup::CleanupGuard;
use crate::audio::processor::frame_pipeline::PreviewAction;
use crate::audio::processor::plan::MediaProcessingPlan;
use crate::audio::SampleRateConfig;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::ProcessingContext;

struct EncodingRunDiagnostics {
    timing: super::run_diagnostics::RunTiming,
    opened_encoder: Option<String>,
    opened_rate: Option<u32>,
    opened_channels: Option<u32>,
    input_facts: Vec<String>,
    encoder_details: Option<String>,
}

/// Processes a single input file through the decode/resample/encode pipeline
/// Returns PreviewAction to signal adaptive preview transitions
pub(crate) fn process_input_file(
    input_path: &Path,
    encoder: &mut super::encoder::EncoderSession,
    file_index: usize,
    ctx: &mut crate::audio::processor::frame_pipeline::FramePipelineCtx,
    accumulator: &mut crate::audio::buffer::SampleAccumulator,
    input_facts: Option<&mut Vec<String>>,
) -> Result<PreviewAction> {
    use crate::errors::AppError;

    let input_label = sanitize_path_for_display(input_path);
    log::info!("🎵 Starting to process input file: {}", input_label);

    if ctx.context.is_cancelled() {
        log::warn!(
            "Processing was cancelled before processing file: {}",
            input_label
        );
        ctx.emitter.emit_cancelled("Processing was cancelled");
        return Err(AppError::cancelled());
    }

    // Initialize per-file preview state if adaptive preview is active
    if let Some(ref mut ps) = ctx.preview_state {
        ps.start_new_file(file_index);
        log::info!(
            "Adaptive preview: starting file {} '{}' at pts={}",
            file_index + 1,
            input_label,
            *ctx.running_pts
        );
    }

    log::info!("Setting up decoder and resampler for: {}", input_label);
    let (mut ictx, mut decoder, mut resampler, stream_index, decode_window) =
        crate::audio::processor::streams::setup_decoder_and_resampler(input_path, encoder)?;
    if let Some(input_facts) = input_facts {
        input_facts.push(format!(
            "file={} codec={:?} rate={} channels={}",
            sanitize_path_for_display(input_path),
            decoder.id(),
            decoder.rate(),
            decoder.channels()
        ));
    }
    log::info!(
        "✓ Decoder and resampler setup complete for stream index: {}",
        stream_index
    );

    // Update context indices for this file
    ctx.current_file_index = file_index;
    ctx.current_stream_index = stream_index;
    log::info!(
        "Updated context: file_index={}, stream_index={}",
        file_index,
        stream_index
    );

    log::info!("Processing input packets from: {}", input_label);
    let action = crate::audio::processor::frame_pipeline::process_input_packets(
        &mut ictx,
        &mut decoder,
        encoder,
        &mut resampler,
        decode_window,
        ctx,
        accumulator,
    )?;
    log::info!(
        "✓ Input packets processed successfully (action={:?})",
        action
    );

    log::info!("✅ Completed processing file: {}", input_label);
    Ok(action)
}

/// Executes a media processing plan through the native ffmpeg-next pipeline.
///
/// Synchronous and CPU-bound; the caller offloads this onto a blocking
/// thread via `spawn_blocking` so it never occupies an async worker.
pub(crate) fn execute(
    plan: &MediaProcessingPlan,
    context: &ProcessingContext,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
) -> Result<()> {
    let mut diagnostics = encoding_log_enabled().then(|| EncodingRunDiagnostics {
        timing: super::run_diagnostics::RunTiming::start(),
        opened_encoder: None,
        opened_rate: None,
        opened_channels: None,
        input_facts: Vec::new(),
        encoder_details: None,
    });
    let result = execute_pipeline(plan, context, metadata, passthrough, diagnostics.as_mut());
    if let Some(diagnostics) = diagnostics {
        append_run_record(|| format_in_process_run(plan, context, &diagnostics, &result));
    }
    result
}

fn execute_pipeline(
    plan: &MediaProcessingPlan,
    context: &ProcessingContext,
    metadata: Option<&crate::metadata::AudiobookMetadata>,
    passthrough: Option<&crate::metadata::PassthroughMetadata>,
    mut diagnostics: Option<&mut EncodingRunDiagnostics>,
) -> Result<()> {
    // Initialize FFmpeg (idempotent)
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = ff::init();
    });

    // Setup encoder and output context with metadata
    // Skip chapter passthrough in preview mode (chapters won't align with shortened output)
    let skip_chapter_passthrough = context.preview.is_some();
    // The session drops its codec/output handles before cleanup removes the path.
    let mut cleanup_guard = CleanupGuard::new(context.session.id());
    cleanup_guard.add_path(&plan.output_path);
    let mut enc_ctx = crate::audio::processor::encoder::setup_encoder(
        plan,
        metadata,
        skip_chapter_passthrough,
        passthrough,
    )?;

    if let Some(diagnostics) = diagnostics.as_deref_mut() {
        diagnostics.opened_encoder = Some(enc_ctx.name().to_owned());
        diagnostics.opened_rate = Some(enc_ctx.rate());
        diagnostics.opened_channels = u32::try_from(enc_ctx.channel_layout().channels()).ok();
    }

    // Validate metadata compatibility if provided
    if let Some(md) = metadata {
        let warnings = crate::metadata::validate_metadata_compatibility(md);
        for warning in warnings {
            log::warn!("Metadata compatibility: {}", warning);
        }
    }

    let emitter = context.new_emitter();
    let result = (|| {
        let mut io = super::engine_orchestrator::InputProcessingContext {
            enc_ctx: &mut enc_ctx,
            emitter: &emitter,
            input_facts: diagnostics
                .as_deref_mut()
                .map(|value| &mut value.input_facts),
        };
        super::engine_orchestrator::process_input_files(plan, context, &mut io)?;
        if context.is_cancelled() {
            return Err(AppError::cancelled());
        }
        enc_ctx.finish()
    })();
    if let Some(diagnostics) = diagnostics {
        diagnostics.encoder_details = Some(enc_ctx.diagnostics());
    }
    result?;

    // Preserve output on success
    let _ = cleanup_guard.remove_path(&plan.output_path);

    if metadata.is_some() {
        log::info!("Audio processing completed with metadata integration");
    } else {
        log::info!("Audio processing completed without metadata");
    }

    Ok(())
}

fn format_in_process_run(
    plan: &MediaProcessingPlan,
    context: &ProcessingContext,
    diagnostics: &EncodingRunDiagnostics,
    result: &Result<()>,
) -> String {
    use std::fmt::Write as _;

    let status = match result {
        Ok(()) => "success",
        Err(AppError::Cancellation(_)) => "cancelled",
        Err(_) => "failed",
    };
    let status_detail = result.as_ref().err().map(ToString::to_string);
    let mut output = String::new();
    write_run_header(
        &mut output,
        "in-process-encoder",
        status,
        status_detail.as_deref(),
    );
    output.push_str("stage=encode_mux\n");
    write_common_run_fields(
        &mut output,
        diagnostics.timing.elapsed(),
        &plan.encoder_settings,
        &plan.sample_rate,
        diagnostics.opened_encoder.as_deref(),
        diagnostics.opened_rate,
        diagnostics.opened_channels,
    );
    write_run_identity(&mut output, context, plan.total_duration);
    let _ = writeln!(output, "preview={}", context.preview.is_some());
    let _ = writeln!(
        output,
        "temp_output={}",
        sanitize_path_for_display(&plan.output_path)
    );
    let _ = writeln!(output, "inputs={}", plan.input_file_paths.len());
    for (index, path) in plan.input_file_paths.iter().enumerate() {
        if let Some(fact) = diagnostics.input_facts.get(index) {
            let _ = writeln!(output, "input[{index}] {fact}");
        } else {
            let _ = writeln!(
                output,
                "input[{index}] file={} codec=unknown rate=unknown channels=unknown",
                sanitize_path_for_display(path)
            );
        }
    }
    if let Some(details) = diagnostics.encoder_details.as_deref() {
        output.push_str(details);
    }
    output.push_str("--- end in-process-encoder run ---\n\n");
    output
}

/// Resolves sample rate after the shared Audio boundary has chosen output channels.
pub(crate) fn resolve_target_audio_params(plan: &MediaProcessingPlan) -> Result<(u32, i32)> {
    let target_channels = plan
        .encoder_settings
        .channels
        .forced_channels()
        .map(i32::from)
        .ok_or_else(|| {
            AppError::General("Output channels were not resolved before encoder setup.".to_string())
        })?;
    let target_sample_rate = match plan.sample_rate {
        SampleRateConfig::Explicit(rate) => rate,
        SampleRateConfig::Auto => {
            let rate = probe_first_sample_rate(plan)?;
            super::super::settings::automatic_sample_rate(
                plan.encoder_settings.encoder_type,
                plan.encoder_settings.faac_profile,
                rate,
            )
        }
    };

    Ok((target_sample_rate, target_channels))
}

fn probe_first_sample_rate(plan: &MediaProcessingPlan) -> Result<u32> {
    let first = plan
        .input_file_paths
        .first()
        .ok_or_else(|| AppError::InvalidInput("No input files provided".to_string()))?;
    let inspection = crate::audio::processor::streams::inspect_audio_decoder(first)?;
    log::info!(
        "probe_first_input path={} selected_decoder={} rate={} channels={}",
        sanitize_path_for_display(first),
        inspection.selected_decoder,
        inspection.sample_rate,
        inspection.channels
    );
    Ok(inspection.sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType};
    use crate::processing::{OutputConfig, ProcessingSession};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn in_process_run_record_sanitizes_paths_and_names_the_encoder() {
        let settings = EncoderSettings {
            encoder_type: EncoderType::AacAt,
            bitrate_mode: BitrateMode::Cvbr,
            channels: ChannelConfig::Stereo,
            ..EncoderSettings::default()
        };
        let plan = MediaProcessingPlan::new(
            PathBuf::from("/private/tmp/worker-output.m4b"),
            settings.clone(),
            SampleRateConfig::Explicit(44_100),
            vec![PathBuf::from("/private/input/Book One.m4b")],
            12.5,
        );
        let context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            settings,
            SampleRateConfig::Explicit(44_100),
            OutputConfig::new(Path::new("/tmp/final/Book.m4b")),
        );
        let mut diagnostics = EncodingRunDiagnostics {
            timing: super::super::run_diagnostics::RunTiming::start(),
            opened_encoder: Some("aac_at".to_string()),
            opened_rate: Some(44_100),
            opened_channels: Some(2),
            input_facts: Vec::new(),
            encoder_details: None,
        };

        let formatted = format_in_process_run(&plan, &context, &diagnostics, &Ok(()));
        assert!(formatted.starts_with("--- in-process-encoder run "));
        assert!(formatted.contains("status=success"));
        assert!(formatted.contains("elapsed_monotonic_ms="));
        assert!(formatted.contains("opened_settings encoder=aac_at rate=44100 channels=2"));
        assert!(formatted.contains("target_duration_seconds=12.500"));
        assert!(formatted.contains("input[0] file=Book One.m4b"));
        assert!(formatted.contains("temp_output=worker-output.m4b"));
        assert!(formatted.ends_with("--- end in-process-encoder run ---\n\n"));
        assert!(!formatted.contains("/private/input"));
        assert!(!formatted.contains("/private/tmp"));

        diagnostics.opened_encoder = None;
        let formatted = format_in_process_run(
            &plan,
            &context,
            &diagnostics,
            &Err(AppError::General("boom".into())),
        );
        assert!(formatted.contains("status=failed"));
        assert!(formatted.contains("status_detail="));
        assert!(formatted.contains("opened_settings encoder=unknown rate=44100 channels=2"));
    }
}
