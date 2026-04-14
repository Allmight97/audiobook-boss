use crate::audio::processor::finalize::commit_output_boundary;
use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::toolchain::ValidatedExternalToolchain;
use crate::audio::CleanupGuard;
use crate::audio::{AudioFile, ProcessingContext, ProgressEmitter};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::ffmpeg_bridge::rewrite_metadata_with_ffmpeg;
use crate::metadata::passthrough::{extract_passthrough_metadata, PassthroughMetadata};
use crate::metadata::AudiobookMetadata;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

pub async fn process_audiobook_with_external_fdk(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
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

    let valid_files: Vec<AudioFile> = files.into_iter().filter(|file| file.is_valid).collect();
    if valid_files.is_empty() {
        return Err(AppError::InvalidInput(
            "No valid audio files found for external FDK processing.".to_string(),
        ));
    }

    let passthrough = if context.preview.is_some() {
        None
    } else {
        let data = extract_passthrough_metadata(&valid_files);
        if data.chapters.is_empty() && data.cover_art.is_none() {
            None
        } else {
            Some(data)
        }
    };

    let effective_metadata = merge_cover_art(metadata, passthrough.as_ref());
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

    if context.is_cancelled() {
        ui.emit_cancelled("Processing was cancelled");
        return Err(AppError::cancelled());
    }

    let destination = preview_output_path(&context);
    ui.emit_cleanup("Cleaning up...");
    let outcome = commit_output_boundary(&context, temp_output, &destination, &mut cleanup_guard)?;
    if outcome.cancelled {
        ui.emit_cancelled("Processing was cancelled");
        return Err(AppError::cancelled());
    }

    ui.emit_complete(if context.preview.is_some() {
        "Preview created successfully"
    } else {
        "Processing complete"
    });

    if context.preview.is_some() {
        Ok(format!(
            "Successfully created preview: {}",
            outcome.final_output.display()
        ))
    } else {
        Ok(format!(
            "Successfully created audiobook: {}",
            outcome.final_output.display()
        ))
    }
}

fn merge_cover_art(
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<&PassthroughMetadata>,
) -> Option<AudiobookMetadata> {
    let passthrough_cover = passthrough
        .and_then(|value| value.cover_art.as_ref())
        .cloned();
    match metadata {
        Some(mut metadata) => {
            if metadata.cover_art.is_none() {
                metadata.cover_art = passthrough_cover;
            }
            Some(metadata)
        }
        None => passthrough_cover.map(|cover_art| {
            let mut metadata = AudiobookMetadata::new();
            metadata.cover_art = Some(cover_art);
            metadata
        }),
    }
}

fn preview_output_path(context: &ProcessingContext) -> PathBuf {
    if context.preview.is_none() {
        return context.output.final_path().to_path_buf();
    }

    let final_output = context.output.final_path();
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    parent.join(format!("{}.preview.m4b", stem))
}

fn create_temp_dir(context: &ProcessingContext) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("abb-fdk-worker-{}", context.session.id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn expected_duration_seconds(
    files: &[AudioFile],
    preview: Option<&crate::audio::preview_config::PreviewConfig>,
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
    temp_output: &Path,
    total_duration_seconds: f64,
) -> Result<()> {
    let mut command = Command::new(&toolchain.ffmpeg_path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.args(build_ffmpeg_args(
        &context.encoder_settings,
        &context.sample_rate,
        context.preview.as_ref(),
        files,
        temp_output,
    ));

    let mut child = command.spawn().map_err(|error| {
        AppError::ProcessTermination(format!(
            "Failed to launch external ffmpeg '{}': {}",
            sanitize_path_for_display(&toolchain.ffmpeg_path),
            error
        ))
    })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stdout was unavailable.".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stderr was unavailable.".to_string())
    })?;

    let mut progress_lines = BufReader::new(stdout).lines();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buffer = String::new();
        let _ = reader.read_to_string(&mut buffer).await;
        buffer
    });

    let total_ms = (total_duration_seconds * 1000.0).max(1.0);
    let current_file = files
        .first()
        .map(|file| sanitize_path_for_display(&file.path));

    loop {
        tokio::select! {
            line = progress_lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(progress_ms) = parse_progress_ms(&line) {
                            let percentage = ((progress_ms / total_ms) * 89.0) as f32;
                            ui.emit_converting_progress(
                                percentage.clamp(1.0, 89.0),
                                "Encoding with external FDK AAC...",
                                current_file.clone(),
                                None,
                            );
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        return Err(AppError::ProcessTermination(format!(
                            "Failed to read external ffmpeg progress: {}",
                            error
                        )));
                    }
                }
            }
            _ = sleep(Duration::from_millis(200)) => {
                if context.is_cancelled() {
                    let _ = child.kill().await;
                    ui.emit_cancelled("Processing was cancelled");
                    return Err(AppError::cancelled());
                }
            }
        }
    }

    let status = child.wait().await?;
    let stderr_output = stderr_task.await.unwrap_or_default();
    if !status.success() {
        let details = stderr_output
            .lines()
            .last()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("External ffmpeg process failed.");
        return Err(AppError::ProcessTermination(details.to_string()));
    }

    ui.emit_converting_progress(89.0, "External FDK encode complete.", current_file, None);
    Ok(())
}

fn build_ffmpeg_args(
    settings: &EncoderSettings,
    sample_rate: &crate::audio::SampleRateConfig,
    preview: Option<&crate::audio::preview_config::PreviewConfig>,
    files: &[AudioFile],
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
    for file in files {
        if let Some(seconds) = preview_per_file.as_ref() {
            args.push("-t".to_string());
            args.push(seconds.clone());
        }
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
    use crate::audio::context::OutputConfig;
    use crate::audio::job_registry::CancellationChecker;
    use crate::audio::session::ProcessingSession;
    use crate::audio::toolchain::EncoderCapabilitySource;
    use crate::audio::AudioFile;
    use std::fs::{set_permissions, write};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tempfile::TempDir;

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
            None,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
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
    async fn worker_cleans_temp_dir_when_external_ffmpeg_fails() {
        let temp_dir = TempDir::new().expect("temp dir");
        let input_path = temp_dir.path().join("input.mp3");
        let output_path = temp_dir.path().join("output.m4b");
        let fake_ffmpeg = write_failing_fake_ffmpeg(temp_dir.path());
        write(&input_path, b"not-real-audio").expect("write fake input");

        let session = Arc::new(ProcessingSession::new());
        let session_id = session.id();
        let expected_worker_temp =
            std::env::temp_dir().join(format!("abb-fdk-worker-{session_id}"));

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
            None,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
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
        let expected_worker_temp =
            std::env::temp_dir().join(format!("abb-fdk-worker-{session_id}"));

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
            Some(AudiobookMetadata {
                title: Some("Trigger rewrite".to_string()),
                ..AudiobookMetadata::default()
            }),
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
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
        let expected_worker_temp =
            std::env::temp_dir().join(format!("abb-fdk-worker-{session_id}"));

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
            None,
            ValidatedExternalToolchain {
                ffmpeg_path: fake_ffmpeg,
                source: EncoderCapabilitySource::Override,
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

    #[test]
    fn progress_parser_handles_ffmpeg_variants() {
        assert_eq!(parse_progress_ms("out_time_ms=1500"), Some(1.5));
        assert_eq!(parse_progress_ms("out_time_us=3200"), Some(3.2));
        assert_eq!(parse_progress_ms("out_time=00:00:02.500000"), Some(2500.0));
        assert_eq!(parse_progress_ms("progress=continue"), None);
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

    fn test_audio_file(path: PathBuf) -> AudioFile {
        AudioFile {
            path,
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
        }
    }
}
