use crate::audio::toolchain::ValidatedExternalToolchain;
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::{ProcessingContext, ProgressEmitter};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

pub(super) async fn run_external_ffmpeg(
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
            super::args::external_input_decoder_name(selection.as_ref()).unwrap_or("auto"),
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
    command.args(super::args::build_ffmpeg_args(
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
                    Ok(Some(line)) => super::progress::emit_external_progress(ui, &line, total_ms, current_file.clone()),
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
