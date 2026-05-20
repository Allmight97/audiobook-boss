use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::toolchain::validate_external_input_decoders;
use crate::audio::toolchain::ValidatedExternalToolchain;
use crate::audio::CleanupGuard;
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::passthrough::{
    extract_passthrough_metadata, merge_passthrough_cover_art, PassthroughMetadata,
};
use crate::metadata::{rewrite_metadata_with_ffmpeg, AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::{ProcessingContext, ProgressEmitter};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

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

    let passthrough = cover_art_passthrough.apply_to_passthrough(collect_passthrough_metadata(
        &valid_files,
        context.preview.is_some(),
    ));

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

    run_external_ffmpeg(
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
        rewrite_metadata_with_ffmpeg(
            &temp_output,
            effective_metadata.as_ref(),
            passthrough.as_ref(),
        )?;
        ui.emit_finalizing("Finalizing metadata...");
    }

    super::finalize::complete_staged_output(&context, temp_output, &mut cleanup_guard, None)
}

fn collect_passthrough_metadata(
    valid_files: &[AudioFile],
    preview: bool,
) -> Option<PassthroughMetadata> {
    let passthrough = extract_passthrough_metadata(valid_files);
    if preview {
        passthrough.cover_art_only()
    } else {
        passthrough.into_option()
    }
}

fn create_temp_dir(context: &ProcessingContext) -> Result<PathBuf> {
    crate::audio::processor::staging::create_destination_staging_dir(
        context.session.uuid(),
        context.output.artifact_path(),
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

async fn run_external_ffmpeg(
    context: &ProcessingContext,
    ui: &ProgressEmitter,
    toolchain: &ValidatedExternalToolchain,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
    total_duration_seconds: f64,
) -> Result<()> {
    log_external_inputs(files, selected_decoders);
    let mut child =
        spawn_external_ffmpeg(context, toolchain, files, selected_decoders, temp_output)?;
    let stdout = take_child_stdout(&mut child)?;
    let stderr = take_child_stderr(&mut child)?;
    let mut progress_lines = BufReader::new(stdout).lines();
    let stderr_task = tokio::spawn(read_stderr_to_string(stderr));
    let current_file = files
        .first()
        .map(|file| sanitize_path_for_display(&file.path));

    let progress_result = monitor_external_progress(
        context,
        ui,
        &mut child,
        &mut progress_lines,
        total_duration_seconds,
        current_file.clone(),
    )
    .await;
    if let Err(error) = progress_result {
        let stderr_output = await_stderr_reader(
            stderr_task,
            "after external ffmpeg progress monitoring ended",
        )
        .await;
        log_external_stderr_on_early_exit(&stderr_output);
        return Err(error);
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            let stderr_output =
                await_stderr_reader(stderr_task, "after external ffmpeg wait failed").await;
            log_external_stderr_on_early_exit(&stderr_output);
            return Err(error.into());
        }
    };
    let stderr_output = await_stderr_reader(stderr_task, "after external ffmpeg exit").await;
    ensure_external_success(status, &stderr_output)?;

    ui.emit_converting_progress(89.0, "External FDK encode complete.", current_file, None);
    Ok(())
}

fn log_external_inputs(files: &[AudioFile], selected_decoders: &[Option<DecoderSelection>]) {
    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        log::info!(
            "external_fdk_input path={} selected_decoder_id={} selected_decoder={} forced_input_decoder={}",
            sanitize_path_for_display(&file.path),
            selection
                .as_ref()
                .map(|value| value.decoder_id.as_str())
                .unwrap_or("unknown"),
            selection
                .as_ref()
                .map(|value| value.decoder_label.as_str())
                .unwrap_or_else(|| file.selected_decoder.as_deref().unwrap_or("unknown")),
            external_input_decoder_name(selection.as_ref()).unwrap_or("auto"),
        );
    }
}

fn spawn_external_ffmpeg(
    context: &ProcessingContext,
    toolchain: &ValidatedExternalToolchain,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
) -> Result<tokio::process::Child> {
    let mut command = Command::new(&toolchain.ffmpeg_path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.args(build_ffmpeg_args(
        &context.encoder_settings,
        &context.sample_rate,
        context.preview.as_ref(),
        files,
        selected_decoders,
        temp_output,
    ));

    command.spawn().map_err(|error| {
        AppError::ProcessTermination(format!(
            "Failed to launch external ffmpeg '{}': {}",
            sanitize_path_for_display(&toolchain.ffmpeg_path),
            error
        ))
    })
}

fn take_child_stdout(child: &mut tokio::process::Child) -> Result<tokio::process::ChildStdout> {
    child.stdout.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stdout was unavailable.".to_string())
    })
}

fn take_child_stderr(child: &mut tokio::process::Child) -> Result<tokio::process::ChildStderr> {
    child.stderr.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stderr was unavailable.".to_string())
    })
}

async fn read_stderr_to_string(stderr: tokio::process::ChildStderr) -> String {
    let mut reader = BufReader::new(stderr);
    let mut buffer = String::new();
    let _ = reader.read_to_string(&mut buffer).await;
    buffer
}

async fn await_stderr_reader(
    stderr_task: tokio::task::JoinHandle<String>,
    context: &str,
) -> String {
    match stderr_task.await {
        Ok(output) => output,
        Err(error) => {
            log::warn!("External ffmpeg stderr reader task failed {context}: {error}");
            String::new()
        }
    }
}

fn log_external_stderr_on_early_exit(stderr_output: &str) {
    if let Some(details) = last_nonempty_stderr_line(stderr_output) {
        log::warn!("External ffmpeg stderr before early exit: {details}");
    }
}

async fn monitor_external_progress<R>(
    context: &ProcessingContext,
    ui: &ProgressEmitter,
    child: &mut tokio::process::Child,
    progress_lines: &mut tokio::io::Lines<R>,
    total_duration_seconds: f64,
    current_file: Option<String>,
) -> Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let total_ms = (total_duration_seconds * 1000.0).max(1.0);
    loop {
        tokio::select! {
            line = progress_lines.next_line() => {
                match line {
                    Ok(Some(line)) => emit_external_progress(ui, &line, total_ms, current_file.clone()),
                    Ok(None) => break,
                    Err(error) => {
                        terminate_external_child_best_effort(
                            child,
                            "after external ffmpeg progress read failure",
                        )
                        .await;
                        return Err(AppError::ProcessTermination(format!(
                            "Failed to read external ffmpeg progress: {}",
                            error
                        )));
                    }
                }
            }
            _ = sleep(Duration::from_millis(200)) => {
                if context.is_cancelled() {
                    terminate_external_child_best_effort(
                        child,
                        "after external ffmpeg cancellation",
                    )
                    .await;
                    ui.emit_cancelled("Processing was cancelled");
                    return Err(AppError::cancelled());
                }
            }
        }
    }
    Ok(())
}

fn emit_external_progress(
    ui: &ProgressEmitter,
    line: &str,
    total_ms: f64,
    current_file: Option<String>,
) {
    if let Some(progress_ms) = parse_progress_ms(line) {
        let percentage = ((progress_ms / total_ms) * 89.0) as f32;
        ui.emit_converting_progress(
            percentage.clamp(1.0, 89.0),
            "Encoding with external FDK AAC...",
            current_file,
            None,
        );
    }
}

fn ensure_external_success(status: std::process::ExitStatus, stderr_output: &str) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    let details =
        last_nonempty_stderr_line(stderr_output).unwrap_or("External ffmpeg process failed.");
    Err(AppError::ProcessTermination(details.to_string()))
}

fn last_nonempty_stderr_line(stderr_output: &str) -> Option<&str> {
    stderr_output
        .lines()
        .rev()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

async fn terminate_external_child_best_effort(child: &mut tokio::process::Child, context: &str) {
    if let Err(error) = terminate_external_child(child).await {
        log::error!("Failed to terminate external ffmpeg child {context}: {error}");
    }
}

async fn terminate_external_child(child: &mut tokio::process::Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    if let Err(error) = child.kill().await {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        return Err(error.into());
    }

    child.wait().await?;
    Ok(())
}

fn build_ffmpeg_args(
    settings: &EncoderSettings,
    sample_rate: &crate::audio::SampleRateConfig,
    preview: Option<&crate::processing::preview_config::PreviewConfig>,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostats".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
    ];

    let preview_per_file = preview.map(|value| value.per_file_seconds(files.len()).to_string());
    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        if let Some(seconds) = preview_per_file.as_ref() {
            args.push("-t".to_string());
            args.push(seconds.clone());
        }
        args.extend(build_input_decoder_args(selection.as_ref()));
        args.push("-i".to_string());
        args.push(file.path.to_string_lossy().to_string());
    }

    args.extend([
        "-map_metadata".to_string(),
        "-1".to_string(),
        "-map_chapters".to_string(),
        "-1".to_string(),
        "-vn".to_string(),
    ]);

    if files.len() > 1 {
        args.push("-filter_complex".to_string());
        args.push(build_concat_filter(files.len()));
        args.push("-map".to_string());
        args.push("[outa]".to_string());
    } else {
        args.push("-map".to_string());
        args.push("0:a:0".to_string());
    }

    args.extend([
        "-c:a".to_string(),
        "libfdk_aac".to_string(),
        "-profile:a".to_string(),
        "aac_he".to_string(),
    ]);

    if let BitrateMode::Vbr(level) = settings.bitrate_mode {
        args.push("-vbr".to_string());
        args.push(level.to_string());
    }

    args.push("-afterburner".to_string());
    args.push(if settings.afterburner { "1" } else { "0" }.to_string());

    if let Some(channels) = settings.channels.forced_channels() {
        args.push("-ac".to_string());
        args.push(channels.to_string());
    }

    if let crate::audio::SampleRateConfig::Explicit(rate) = sample_rate {
        args.push("-ar".to_string());
        args.push(rate.to_string());
    }

    match settings.threads {
        ThreadSetting::Auto => {}
        ThreadSetting::Off => {
            args.push("-threads".to_string());
            args.push("1".to_string());
        }
        ThreadSetting::Fixed(value) => {
            args.push("-threads".to_string());
            args.push(value.to_string());
        }
    }

    args.push(temp_output.to_string_lossy().to_string());
    args
}

fn build_input_decoder_args(selection: Option<&DecoderSelection>) -> Vec<String> {
    let Some(decoder_name) = external_input_decoder_name(selection) else {
        return Vec::new();
    };

    vec!["-c:a".to_string(), decoder_name.to_string()]
}

fn external_input_decoder_name(selection: Option<&DecoderSelection>) -> Option<&str> {
    match selection.map(|value| value.decoder_id.as_str()) {
        Some("aac_at") => Some("aac_at"),
        Some("libfdk_aac") => Some("libfdk_aac"),
        _ => None,
    }
}

fn build_concat_filter(input_count: usize) -> String {
    let mut filter = String::new();
    for index in 0..input_count {
        filter.push_str(&format!("[{}:a:0]", index));
    }
    filter.push_str(&format!("concat=n={}:v=0:a=1[outa]", input_count));
    filter
}

fn parse_progress_ms(line: &str) -> Option<f64> {
    let (_, raw) = line.split_once('=')?;
    if line.starts_with("out_time_ms=") || line.starts_with("out_time_us=") {
        return raw.parse::<f64>().ok().map(|value| value / 1000.0);
    }

    if line.starts_with("out_time=") {
        let mut parts = raw.split(':');
        let hours = parts.next()?.parse::<f64>().ok()?;
        let minutes = parts.next()?.parse::<f64>().ok()?;
        let seconds = parts.next()?.parse::<f64>().ok()?;
        return Some((((hours * 60.0) + minutes) * 60.0 + seconds) * 1000.0);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::toolchain::EncoderCapabilitySource;
    use crate::audio::AudioFile;
    use crate::commands::metadata::{read_audio_metadata, save_metadata_to_file};
    use crate::processing::context::OutputConfig;
    use crate::processing::job_registry::CancellationChecker;
    use crate::processing::session::ProcessingSession;
    use ffmpeg_next as ff;
    use std::fs::{set_permissions, write};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tempfile::TempDir;

    const MINIMAL_JPEG: &[u8] = include_bytes!("../../../tests/support/minimal.jpg");

    fn expected_worker_staging_dir(output_path: &Path, session_id: &str) -> PathBuf {
        output_path
            .parent()
            .expect("output path should have parent")
            .join(format!(".abb-processing-{session_id}"))
    }

    #[tokio::test]
    async fn worker_processes_with_fake_external_ffmpeg() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
        write(&input_path, b"not-real-audio").expect("write fake input");

        let context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: crate::audio::settings_encoder::ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&output_path),
        );

        let result = process_audiobook_with_external_fdk(
            context,
            vec![AudioFile {
                path: input_path.clone(),
                size: Some(1.0),
                duration: Some(5.0),
                format: Some("MP3".to_string()),
                bitrate: None,
                sample_rate: None,
                channels: None,
                codec_label: None,
                selected_decoder: None,
                is_valid: true,
                error: None,
            }],
            vec![None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await
        .expect("external worker should succeed");

        assert!(result.contains("Successfully created audiobook"));
        assert!(
            output_path.exists(),
            "expected worker output at {:?}",
            output_path
        );
    }

    #[tokio::test]
    async fn worker_preview_suppresses_passthrough_chapters_while_full_keeps_them() {
        let temp_dir = TempDir::new().expect("temp dir");
        let first_input = temp_dir.path().join("input-one.m4b");
        let second_input = temp_dir.path().join("input-two.m4b");
        let template_output = temp_dir.path().join("template.m4b");
        let preview_output = temp_dir.path().join("output.preview.m4b");
        let full_output = temp_dir.path().join("output.m4b");
        write_minimal_m4b(&first_input);
        write_minimal_m4b(&second_input);
        write_minimal_m4b(&template_output);
        save_metadata_to_file(
            first_input.to_string_lossy().to_string(),
            AudiobookMetadata {
                cover_art: Some(MINIMAL_JPEG.to_vec()),
                ..AudiobookMetadata::default()
            }
            .into(),
        )
        .await
        .expect("seed cover art on first source");
        let fake_ffmpeg = write_copying_fake_ffmpeg(temp_dir.path(), &template_output);

        let mut preview_context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            fdk_test_settings(),
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::for_preview(&preview_output),
        );
        preview_context.preview = Some(crate::processing::context::PreviewConfig::new(30.0));

        let preview_result = process_audiobook_with_external_fdk(
            preview_context,
            vec![
                test_audio_file(first_input.clone()),
                test_audio_file(second_input.clone()),
            ],
            vec![None, None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg.clone(),
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await
        .expect("external preview worker should succeed");

        assert!(preview_result.contains("Successfully created preview"));
        assert!(preview_output.exists(), "expected preview output to exist");
        assert_cover_art_and_chapter_count(
            &preview_output,
            0,
            "external preview should not preserve passthrough chapters",
        )
        .await;

        let full_context = ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            fdk_test_settings(),
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&full_output),
        );

        let full_result = process_audiobook_with_external_fdk(
            full_context,
            vec![test_audio_file(first_input), test_audio_file(second_input)],
            vec![None, None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await
        .expect("external full worker should succeed");

        assert!(full_result.contains("Successfully created audiobook"));
        assert!(full_output.exists(), "expected full output to exist");
        assert_cover_art_and_chapter_count(
            &full_output,
            2,
            "external full run should keep synthesized passthrough chapters for both inputs",
        )
        .await;
    }

    #[tokio::test]
    async fn worker_cleans_temp_dir_when_external_ffmpeg_fails() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let fake_ffmpeg = write_failing_fake_ffmpeg(temp_dir.path());
        write(&input_path, b"not-real-audio").expect("write fake input");

        let session = Arc::new(ProcessingSession::new());
        let session_id = session.id();
        let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

        let context = ProcessingContext::new_headless(
            session,
            EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: crate::audio::settings_encoder::ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&output_path),
        );

        let result = process_audiobook_with_external_fdk(
            context,
            vec![test_audio_file(input_path.clone())],
            vec![None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await;

        assert!(result.is_err(), "ffmpeg failure should propagate");
        assert!(
            !expected_worker_temp.exists(),
            "worker temp dir should be cleaned on ffmpeg failure: {:?}",
            expected_worker_temp
        );
        assert!(
            !output_path.exists(),
            "final output should not exist on ffmpeg failure"
        );
    }

    #[tokio::test]
    async fn worker_cleans_temp_dir_when_metadata_rewrite_fails() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
        write(&input_path, b"not-real-audio").expect("write fake input");

        let session = Arc::new(ProcessingSession::new());
        let session_id = session.id();
        let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

        let context = ProcessingContext::new_headless(
            session,
            EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: crate::audio::settings_encoder::ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&output_path),
        );

        let result = process_audiobook_with_external_fdk(
            context,
            vec![test_audio_file(input_path.clone())],
            vec![None],
            Some(AudiobookMetadata {
                title: Some("Trigger rewrite".to_string()),
                ..AudiobookMetadata::default()
            }),
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await;

        assert!(result.is_err(), "metadata rewrite failure should propagate");
        assert!(
            !expected_worker_temp.exists(),
            "worker temp dir should be cleaned on metadata rewrite failure"
        );
        assert!(
            !output_path.exists(),
            "final output should not exist when metadata rewrite fails"
        );
    }

    #[tokio::test]
    async fn worker_returns_cancelled_without_success_emit_for_late_cancel_before_commit() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
        write(&input_path, b"not-real-audio").expect("write fake input");

        let job_flag = Arc::new(AtomicBool::new(true));
        let checker = CancellationChecker {
            job_flag,
            global_flag: Arc::new(AtomicBool::new(false)),
        };
        let session = Arc::new(ProcessingSession::from_job_registry(
            uuid::Uuid::new_v4(),
            checker,
        ));
        let session_id = session.id();
        let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

        let context = ProcessingContext::new_headless(
            session,
            EncoderSettings {
                encoder_type: EncoderType::FdkHeAac,
                bitrate_kbps: 64,
                bitrate_mode: BitrateMode::Vbr(3),
                channels: crate::audio::settings_encoder::ChannelConfig::Auto,
                afterburner: true,
                threads: ThreadSetting::Auto,
                twoloop: true,
            },
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&output_path),
        );

        let result = process_audiobook_with_external_fdk(
            context,
            vec![test_audio_file(input_path.clone())],
            vec![None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await;

        let error = result.expect_err("late cancellation should return error");
        assert!(
            matches!(&error, crate::errors::AppError::Cancellation(_)),
            "expected dedicated cancellation error, got: {error}"
        );
        assert!(
            format!("{error}").contains("Processing was cancelled"),
            "expected cancelled error, got: {error}"
        );
        assert!(
            !output_path.exists(),
            "no final output should be reported as complete on cancelled path"
        );
        assert!(
            !expected_worker_temp.exists(),
            "worker temp dir should be cleaned on cancelled path"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_reaps_child_when_progress_stream_is_invalid_utf8() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let pid_file = temp_dir.path().join("fake-ffmpeg.pid");
        let fake_ffmpeg = write_invalid_utf8_sleeping_fake_ffmpeg(temp_dir.path(), &pid_file);
        write(&input_path, b"not-real-audio").expect("write fake input");

        let session = Arc::new(ProcessingSession::new());
        let session_id = session.id();
        let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

        let context = ProcessingContext::new_headless(
            session,
            fdk_test_settings(),
            crate::audio::SampleRateConfig::Auto,
            OutputConfig::new(&output_path),
        );

        let result = process_audiobook_with_external_fdk(
            context,
            vec![test_audio_file(input_path)],
            vec![None],
            None,
            CoverArtPassthroughPolicy::Preserve,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
                decoder_capabilities: Default::default(),
            },
        )
        .await;

        let error = result.expect_err("invalid utf-8 progress should fail");
        assert!(
            error
                .to_string()
                .contains("Failed to read external ffmpeg progress"),
            "unexpected error: {error}"
        );

        let pid = std::fs::read_to_string(&pid_file)
            .expect("pid file should exist")
            .trim()
            .parse::<u32>()
            .expect("pid file should contain numeric pid");
        assert_process_exited(pid).await;
        assert!(
            !expected_worker_temp.exists(),
            "worker temp dir should be cleaned after progress read failure"
        );
        assert!(
            !output_path.exists(),
            "final output should not exist after progress read failure"
        );
    }

    #[test]
    fn progress_parser_handles_ffmpeg_variants() {
        assert_eq!(parse_progress_ms("out_time_ms=1500"), Some(1.5));
        assert_eq!(parse_progress_ms("out_time_us=3200"), Some(3.2));
        assert_eq!(parse_progress_ms("out_time=00:00:02.500000"), Some(2500.0));
        assert_eq!(parse_progress_ms("progress=continue"), None);
    }

    #[test]
    fn build_ffmpeg_args_forces_selected_input_decoder_before_each_input() {
        let output_path = PathBuf::from("/tmp/output.m4b");
        let first_input = PathBuf::from("/tmp/first.m4b");
        let second_input = PathBuf::from("/tmp/second.m4b");
        let third_input = PathBuf::from("/tmp/third.m4b");
        let settings = EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        };

        let args = build_ffmpeg_args(
            &settings,
            &crate::audio::SampleRateConfig::Auto,
            None,
            &[
                audio_file_with_decoder(&first_input, Some("Apple AAC")),
                audio_file_with_decoder(&second_input, None),
                audio_file_with_decoder(&third_input, Some("FDK AAC")),
            ],
            &[
                decoder_selection("aac_at", "Apple AAC"),
                None,
                decoder_selection("libfdk_aac", "FDK AAC"),
            ],
            &output_path,
        );

        assert!(args.windows(4).any(|window| {
            window
                == [
                    "-c:a".to_string(),
                    "aac_at".to_string(),
                    "-i".to_string(),
                    first_input.to_string_lossy().to_string(),
                ]
        }));
        assert!(args.windows(2).any(|window| {
            window == ["-i".to_string(), second_input.to_string_lossy().to_string()]
        }));
        assert!(args.windows(4).any(|window| {
            window
                == [
                    "-c:a".to_string(),
                    "libfdk_aac".to_string(),
                    "-i".to_string(),
                    third_input.to_string_lossy().to_string(),
                ]
        }));
        assert!(args
            .windows(2)
            .any(|window| { window == ["-c:a".to_string(), "libfdk_aac".to_string()] }));
    }

    #[test]
    fn build_ffmpeg_args_keeps_default_decoder_inputs_unforced() {
        let output_path = PathBuf::from("/tmp/output.m4b");
        let input_path = PathBuf::from("/tmp/input.m4b");
        let settings = EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: false,
            threads: ThreadSetting::Auto,
            twoloop: true,
        };

        let args = build_ffmpeg_args(
            &settings,
            &crate::audio::SampleRateConfig::Auto,
            None,
            &[audio_file_with_decoder(
                &input_path,
                Some("Native AAC (FFmpeg)"),
            )],
            &[decoder_selection("default", "Renamed display label")],
            &output_path,
        );

        let input_index = args
            .iter()
            .position(|value| value == "-i")
            .expect("input flag present");
        assert!(input_index >= 1, "expected option before -i");
        assert_ne!(args[input_index - 1], "aac_at");
        assert_ne!(args[input_index - 1], "libfdk_aac");
    }

    #[test]
    fn build_ffmpeg_args_depend_on_decoder_id_not_friendly_label() {
        let output_path = PathBuf::from("/tmp/output.m4b");
        let input_path = PathBuf::from("/tmp/input.m4b");
        let settings = EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: false,
            threads: ThreadSetting::Auto,
            twoloop: true,
        };

        let args = build_ffmpeg_args(
            &settings,
            &crate::audio::SampleRateConfig::Auto,
            None,
            &[audio_file_with_decoder(
                &input_path,
                Some("Renamed Apple Label"),
            )],
            &[decoder_selection("aac_at", "Renamed Apple Label")],
            &output_path,
        );

        assert!(args.windows(4).any(|window| {
            window
                == [
                    "-c:a".to_string(),
                    "aac_at".to_string(),
                    "-i".to_string(),
                    input_path.to_string_lossy().to_string(),
                ]
        }));
    }

    #[test]
    fn collect_passthrough_metadata_keeps_cover_art_but_drops_preview_chapters() {
        let input_path = PathBuf::from("/tmp/input.m4b");
        let passthrough = collect_passthrough_metadata(
            &[AudioFile {
                path: input_path,
                size: Some(1.0),
                duration: Some(5.0),
                format: Some("M4B".to_string()),
                bitrate: None,
                sample_rate: None,
                channels: None,
                codec_label: None,
                selected_decoder: None,
                is_valid: false,
                error: None,
            }],
            true,
        );

        assert!(
            passthrough.is_none(),
            "invalid files should not create passthrough state"
        );

        let cover_only = PassthroughMetadata {
            chapters: vec![crate::metadata::passthrough::ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: Some(vec![1, 2, 3]),
        }
        .cover_art_only()
        .expect("cover-only passthrough");
        assert!(cover_only.chapters.is_empty());
        assert_eq!(cover_only.cover_art, Some(vec![1, 2, 3]));
    }

    #[test]
    fn merge_passthrough_cover_art_does_not_refill_when_passthrough_cover_is_filtered() {
        let metadata = AudiobookMetadata {
            cover_art: None,
            ..AudiobookMetadata::default()
        };
        let passthrough = PassthroughMetadata {
            chapters: Vec::new(),
            cover_art: None,
        };

        let merged = merge_passthrough_cover_art(Some(metadata), Some(&passthrough))
            .expect("metadata should remain present");

        assert_eq!(merged.cover_art, None);
    }

    fn fdk_test_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        }
    }

    async fn assert_cover_art_and_chapter_count(
        output_path: &std::path::Path,
        expected_chapters: usize,
        chapter_context: &str,
    ) {
        let read_back = read_audio_metadata(output_path.to_string_lossy().to_string())
            .await
            .expect("read output metadata");
        assert_eq!(
            read_back.cover_art.as_deref(),
            Some(MINIMAL_JPEG),
            "output should preserve exact source-derived cover art bytes"
        );
        assert_eq!(
            chapter_count(output_path),
            expected_chapters,
            "{chapter_context}"
        );
    }

    fn write_minimal_m4b(output: &std::path::Path) {
        let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
        let mut octx = ff::format::output(output).expect("create output context");
        let time_base = ff::Rational(1, 44_100);

        let mut enc_ctx = ff::codec::context::Context::new()
            .encoder()
            .audio()
            .expect("encoder context");
        enc_ctx.set_rate(44_100);
        enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
        enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
        enc_ctx.set_time_base(time_base);
        let mut enc = enc_ctx.open_as(codec).expect("open encoder");

        let (stream_index, stream_time_base) = {
            let mut ost = octx.add_stream(codec).expect("add stream");
            ost.set_time_base(enc.time_base());
            ost.set_parameters(&enc);
            (ost.index(), ost.time_base())
        };
        octx.write_header().expect("write header");

        let mut frame = ff::frame::Audio::empty();
        frame.set_format(enc.format());
        frame.set_channel_layout(enc.channel_layout());
        frame.set_rate(enc.rate());
        frame.set_samples(1024);
        unsafe {
            frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
        }
        frame.set_pts(Some(0));
        let plane = frame.data_mut(0);
        let samples: &mut [f32] = unsafe {
            std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples())
        };
        samples.fill(0.0);

        let mut pkt = ff::Packet::empty();
        enc.send_frame(&frame).expect("send frame");
        while enc.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(stream_index);
            pkt.rescale_ts(enc.time_base(), stream_time_base);
            pkt.write_interleaved(&mut octx).expect("write packet");
        }

        enc.send_eof().ok();
        while enc.receive_packet(&mut pkt).is_ok() {
            pkt.set_stream(stream_index);
            pkt.rescale_ts(enc.time_base(), stream_time_base);
            pkt.write_interleaved(&mut octx).expect("write packet");
        }
        octx.write_trailer().expect("write trailer");
    }

    fn chapter_count(path: &std::path::Path) -> usize {
        ff::init().expect("ffmpeg init");
        let ictx = ff::format::input(path).expect("open output for chapter inspection");
        ictx.nb_chapters() as usize
    }

    fn write_fake_ffmpeg(root: &Path) -> PathBuf {
        let script_path = root.join("fake-ffmpeg");
        let script = "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\necho 'out_time_ms=5000'\n: > \"$last\"\nexit 0\n";
        write(&script_path, script).expect("write fake ffmpeg");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod fake ffmpeg");
        script_path
    }

    fn write_copying_fake_ffmpeg(root: &Path, template: &Path) -> PathBuf {
        let script_path = root.join("fake-ffmpeg-copy");
        let script = format!(
            "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\necho 'out_time_ms=5000'\ncp '{}' \"$last\"\nexit 0\n",
            template.display()
        );
        write(&script_path, script).expect("write fake ffmpeg copy script");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod fake ffmpeg copy script");
        script_path
    }

    fn write_failing_fake_ffmpeg(root: &Path) -> PathBuf {
        let script_path = root.join("fake-ffmpeg-fail");
        let script = "#!/bin/sh\necho 'out_time_ms=2000'\necho 'forced failure' 1>&2\nexit 1\n";
        write(&script_path, script).expect("write failing fake ffmpeg");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod failing fake ffmpeg");
        script_path
    }

    #[cfg(unix)]
    fn write_invalid_utf8_sleeping_fake_ffmpeg(root: &Path, pid_file: &Path) -> PathBuf {
        let script_path = root.join("fake-ffmpeg-invalid-utf8");
        let script = format!(
            "#!/bin/sh\necho \"$$\" > '{}'\nprintf '\\377\\n'\nexec sleep 30\n",
            pid_file.display()
        );
        write(&script_path, script).expect("write invalid utf8 fake ffmpeg");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod invalid utf8 fake ffmpeg");
        script_path
    }

    #[cfg(unix)]
    async fn assert_process_exited(pid: u32) {
        for _ in 0..20 {
            if !process_is_alive(pid) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        assert!(
            !process_is_alive(pid),
            "expected external ffmpeg child {pid} to be reaped"
        );
    }

    #[cfg(unix)]
    fn process_is_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn test_audio_file(path: PathBuf) -> AudioFile {
        audio_file_with_decoder(&path, None)
    }

    fn decoder_selection(decoder_id: &str, decoder_label: &str) -> Option<DecoderSelection> {
        Some(DecoderSelection {
            decoder_id: decoder_id.to_string(),
            decoder_label: decoder_label.to_string(),
        })
    }

    fn audio_file_with_decoder(path: &Path, selected_decoder: Option<&str>) -> AudioFile {
        AudioFile {
            path: path.to_path_buf(),
            size: Some(1.0),
            duration: Some(5.0),
            format: Some("MP3".to_string()),
            bitrate: None,
            sample_rate: None,
            channels: None,
            codec_label: None,
            selected_decoder: selected_decoder.map(ToOwned::to_owned),
            is_valid: true,
            error: None,
        }
    }
}
